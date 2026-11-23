//! Loopback-only login and character selection for the 2026-02-19 client.
use rand::Rng;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::protocol::{self, CharacterEntry, CharacterRequest};

const ACCOUNT: u32 = 2_000_001;
const CREDENTIAL: &[u8] = b"tester2";
const CHAR_PORT: u16 = 6121;
const SLOT_COUNT: u8 = 15;
const SEX: u8 = 1;

type Sessions = Arc<Mutex<Option<Session>>>;

struct Session {
    id1: u32,
    id2: u32,
    expires_at: Instant,
}

pub fn run() -> io::Result<()> {
    let login = TcpListener::bind((Ipv4Addr::LOCALHOST, 6900))?;
    let characters = TcpListener::bind((Ipv4Addr::LOCALHOST, CHAR_PORT))?;
    let sessions = Sessions::default();
    let login_sessions = sessions.clone();
    thread::spawn(move || serve(login, login_sessions, handle_login));
    println!("Local demo listening on 127.0.0.1:6900 and :6121");
    serve(characters, sessions, handle_characters)
}

fn serve(
    listener: TcpListener,
    sessions: Sessions,
    handle: fn(TcpStream, &Sessions) -> io::Result<()>,
) -> io::Result<()> {
    for stream in listener.incoming() {
        let stream = stream?;
        let sessions = sessions.clone();
        thread::spawn(move || {
            if let Err(error) = handle(stream, &sessions) {
                if !matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof | io::ErrorKind::TimedOut
                ) {
                    eprintln!("Local client disconnected: {}", error);
                }
            }
        });
    }
    Ok(())
}

fn handle_login(mut stream: TcpStream, sessions: &Sessions) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let request = protocol::read_login(&mut stream)?;
    if request.username() != CREDENTIAL || request.password() != CREDENTIAL {
        return stream.write_all(&protocol::login_refused());
    }

    let mut rng = rand::thread_rng();
    let session = Session {
        id1: rng.gen(),
        id2: rng.gen(),
        expires_at: Instant::now() + Duration::from_secs(60),
    };
    let reply = protocol::login_reply(
        ACCOUNT,
        [session.id1, session.id2],
        SEX,
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, CHAR_PORT),
        b"Rust Athena",
    )?;
    *sessions.lock().expect("session lock poisoned") = Some(session);
    stream.write_all(&reply)?;
    // The client closes the login connection after selecting a character server.
    // Sending EOF with the reply can make the client's update loop discard it.
    io::copy(&mut stream, &mut io::sink())?;
    Ok(())
}

fn handle_characters(mut stream: TcpStream, sessions: &Sessions) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(90)))?;
    let request = protocol::read_character_entry(&mut stream)?;
    if !consume_session(sessions, &request) {
        return stream.write_all(&protocol::character_entry_refused(ACCOUNT));
    }

    let character = character_record();
    stream.write_all(&protocol::character_entry(ACCOUNT, SLOT_COUNT, &character))?;
    let page = protocol::character_page(&character);
    loop {
        match protocol::read_character_request(&mut stream)? {
            CharacterRequest::Page => stream.write_all(&page)?,
            CharacterRequest::Keepalive => {}
        }
    }
}

fn consume_session(sessions: &Sessions, request: &CharacterEntry) -> bool {
    let mut pending = sessions.lock().expect("session lock poisoned");
    let valid = pending.as_ref().is_some_and(|session| {
        Instant::now() < session.expires_at
            && request.account == ACCOUNT
            && request.login_ids == [session.id1, session.id2]
            && request.sex == SEX
    });
    if valid {
        *pending = None;
    }
    valid
}

fn character_record() -> protocol::CharacterRecord {
    protocol::CharacterSummary {
        id: 150_000,
        job_level: 1,
        hp: 40,
        max_hp: 40,
        sp: 11,
        max_sp: 11,
        walk_speed: 150,
        base_level: 1,
        name: b"Ferris",
        rename_count: 1,
        map: b"prontera.gat",
        sex: SEX,
    }
    .encode()
    .expect("demo character fields fit their wire widths")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_page_matches_reference() {
        let hex = include_str!("reference/character-page.hex").trim();
        let expected: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect();
        let character = character_record();
        assert_eq!(protocol::character_page(&character), expected);
        let entry = protocol::character_entry(ACCOUNT, SLOT_COUNT, &character);
        let page_notice = 4 + 29 + 27 + character.len();
        assert_eq!(
            &entry[page_notice..page_notice + 6],
            &[0xa0, 0x09, 1, 0, 0, 0]
        );
    }

    #[test]
    fn login_and_character_list_work_across_connections() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (server, _) = listener.accept().unwrap();
        let sessions = Sessions::default();
        let login_sessions = sessions.clone();
        let handler = thread::spawn(move || handle_login(server, &login_sessions));

        let mut digest_and_login = vec![0; 18 + 55];
        digest_and_login[..2].copy_from_slice(&0x0204u16.to_le_bytes());
        digest_and_login[18..20].copy_from_slice(&0x0064u16.to_le_bytes());
        digest_and_login[24..31].copy_from_slice(CREDENTIAL);
        digest_and_login[48..55].copy_from_slice(CREDENTIAL);
        client.write_all(&digest_and_login[..1]).unwrap();
        client.write_all(&digest_and_login[1..]).unwrap();

        let mut reply = [0; 224];
        client.read_exact(&mut reply).unwrap();
        assert_eq!(&reply[..4], &[0xc4, 0x0a, 224, 0]);
        drop(client);
        handler.join().unwrap().unwrap();

        let mut character_client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        character_client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (character_server, _) = listener.accept().unwrap();
        let character_handler =
            thread::spawn(move || handle_characters(character_server, &sessions));

        let mut entry = [0; 17];
        entry[..2].copy_from_slice(&0x0065u16.to_le_bytes());
        entry[2..6].copy_from_slice(&reply[8..12]);
        entry[6..10].copy_from_slice(&reply[4..8]);
        entry[10..14].copy_from_slice(&reply[12..16]);
        entry[16] = reply[46];
        character_client.write_all(&entry).unwrap();

        let mut roster = [0; 257];
        character_client.read_exact(&mut roster).unwrap();
        assert_eq!(&roster[..4], &ACCOUNT.to_le_bytes());
        assert_eq!(&roster[33..37], &[0x6b, 0, 202, 0]);
        character_client
            .write_all(&0x09a1u16.to_le_bytes())
            .unwrap();
        let mut page = [0; 179];
        character_client.read_exact(&mut page).unwrap();
        assert_eq!(&page[..4], &[0x72, 0x0b, 179, 0]);
        assert_eq!(&page[4..], &roster[60..235]);
        drop(character_client);
        assert_eq!(
            character_handler.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn character_handoff_rejects_invalid_and_replayed_session() {
        let sessions = Sessions::default();
        *sessions.lock().unwrap() = Some(Session {
            id1: 1,
            id2: 2,
            expires_at: Instant::now() + Duration::from_secs(60),
        });
        let mut request = CharacterEntry {
            account: ACCOUNT,
            login_ids: [1, 0],
            sex: SEX,
        };
        assert!(!consume_session(&sessions, &request));
        request.login_ids[1] = 2;
        assert!(consume_session(&sessions, &request));
        assert!(!consume_session(&sessions, &request));
    }
}
