use std::convert::TryFrom;
use std::io::{self, Write};
use std::net::{SocketAddrV4, TcpStream};
use std::time::Duration;

use rust_athena_db::{Character, CharacterCredentials, Database, NewCharacter};

use crate::{listener, map_server::START_MAP, protocol::v20260219 as wire, Result};

const SLOTS: u8 = 15;

pub fn serve(db: Database, bind: SocketAddrV4, map_endpoint: SocketAddrV4) -> Result<()> {
    listener::serve(bind, Duration::from_secs(90), move |stream| {
        handle(&db, stream, map_endpoint)
    })
}

fn handle(db: &Database, mut stream: TcpStream, map_endpoint: SocketAddrV4) -> Result<()> {
    let entry = wire::read_character_entry(&mut stream)?;
    let account = entry.account;
    if !db.redeem_handoff(account, entry.login_ids, entry.sex)? {
        stream.write_all(&wire::character_entry_refused(account))?;
        return Ok(());
    }

    let mut roster = db.characters(account)?;
    let mut records = roster.iter().map(record).collect::<io::Result<Vec<_>>>()?;
    stream.write_all(&wire::character_entry(account, SLOTS, &records))?;
    loop {
        match wire::read_character_request(&mut stream)? {
            wire::CharacterRequest::Page => stream.write_all(&wire::character_page(&records))?,
            wire::CharacterRequest::Keepalive(id) if id == account => {}
            wire::CharacterRequest::Keepalive(_) => return Ok(()),
            wire::CharacterRequest::Create(request) => {
                match create(db, account, &roster, request)? {
                    Ok(character) => {
                        let encoded = record(&character)?;
                        stream.write_all(&wire::character_created(&encoded))?;
                        let index = roster.partition_point(|item| item.slot < character.slot);
                        roster.insert(index, character);
                        records.insert(index, encoded);
                    }
                    Err(reason) => stream.write_all(&wire::character_creation_refused(reason))?,
                }
            }
            wire::CharacterRequest::Select(slot) => {
                let character = match roster.iter().find(|character| character.slot == slot) {
                    Some(character) => character,
                    None => {
                        stream.write_all(&wire::character_selection_refused(false))?;
                        continue;
                    }
                };
                if character.map_name != START_MAP
                    || !db.issue_map_transfer(
                        account,
                        character.id,
                        CharacterCredentials {
                            login_ids: entry.login_ids,
                            sex: entry.sex,
                        },
                        START_MAP,
                    )?
                {
                    stream.write_all(&wire::character_selection_refused(true))?;
                    continue;
                }
                stream.write_all(&wire::map_destination(
                    character.id,
                    START_MAP.as_bytes(),
                    map_endpoint,
                )?)?;
                // Rodecomp opens the map connection after reading this response.
                // Keep the character socket open until the client closes it.
                stream.set_read_timeout(Some(Duration::from_secs(10)))?;
                return listener::drain(stream);
            }
        }
    }
}

fn create(
    db: &Database,
    account: u32,
    roster: &[Character],
    request: wire::CreateCharacter,
) -> Result<std::result::Result<Character, u8>> {
    if request.slot >= SLOTS
        || roster
            .iter()
            .any(|character| character.slot == request.slot)
    {
        return Ok(Err(3));
    }
    let name = match std::str::from_utf8(request.name()) {
        Ok(name)
            if (4..=23).contains(&name.len())
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') =>
        {
            name
        }
        _ => return Ok(Err(12)),
    };
    let job = match i16::try_from(request.job) {
        Ok(job @ (0 | 4218)) => job,
        _ => return Ok(Err(4)),
    };
    if request.sex > 1 || request.hair < 0 || request.hair_color < 0 {
        return Ok(Err(4));
    }
    let created = db.create_character(NewCharacter {
        account_id: i64::from(account),
        slot: request.slot,
        name,
        job,
        hair: request.hair,
        hair_color: request.hair_color,
        sex: request.sex,
        map_name: START_MAP,
    })?;
    Ok(created.ok_or(0))
}

fn record(character: &Character) -> io::Result<wire::CharacterRecord> {
    wire::CharacterSummary {
        id: character.id,
        job_level: 1,
        hp: 40,
        max_hp: 40,
        sp: 11,
        max_sp: 11,
        walk_speed: 150,
        base_level: 1,
        name: character.name.as_bytes(),
        slot: character.slot,
        job: character.job,
        hair: character.hair,
        hair_color: character.hair_color,
        map: character.map_name.as_bytes(),
        sex: character.sex,
    }
    .encode()
}
