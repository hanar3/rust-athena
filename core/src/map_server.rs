use std::net::{Shutdown, SocketAddrV4, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use std::{env, fs, io::Write};

use rust_athena_db::Database;

use crate::{
    listener,
    map_sessions::{MapSessions, Session},
    protocol::v20260219 as wire,
    terrain::{Terrain, Tile},
    Result,
};

pub const START_MAP: &str = "prontera.gat";
const SPAWN_X: u16 = 156;
const SPAWN_Y: u16 = 191;
const SPAWN_DIRECTION: u8 = 4;
const LOAD_TIMEOUT: Duration = Duration::from_secs(90);

pub fn serve(db: Database, bind: SocketAddrV4) -> Result<()> {
    let terrain = env::var_os("ATHENA_MAP_CACHE")
        .map(|path| -> Result<Terrain> {
            let bytes = fs::read(&path)
                .map_err(|error| format!("ATHENA_MAP_CACHE {:?}: {}", path, error))?;
            let map = START_MAP.strip_suffix(".gat").unwrap_or(START_MAP);
            Ok(Terrain::from_cache(&bytes, map)?)
        })
        .transpose()?;
    if let Some(terrain) = &terrain {
        if !terrain.walkable((SPAWN_X, SPAWN_Y)) {
            return Err("Configured terrain blocks the Prontera spawn".into());
        }
        println!("Loaded walkability for {}", START_MAP);
    } else {
        println!("Movement disabled. Set ATHENA_MAP_CACHE to enable it.");
    }
    let started = Instant::now();
    let sessions = MapSessions::start(started, terrain)?;
    listener::serve(bind, Duration::from_secs(10), move |stream| {
        handle(&db, stream, started, &sessions)
    })
}

fn handle(
    db: &Database,
    mut stream: TcpStream,
    started: Instant,
    sessions: &MapSessions,
) -> Result<()> {
    let entry = wire::read_map_entry(&mut stream)?;
    if !db.redeem_map_transfer(
        entry.account,
        entry.character,
        entry.auth_code,
        entry.sex,
        START_MAP,
    )? {
        stream.write_all(&wire::map_entry_refused())?;
        return listener::drain(stream);
    }

    let Some(character) = db.character(entry.account, entry.character)? else {
        stream.write_all(&wire::map_entry_refused())?;
        return listener::drain(stream);
    };
    let appearance = wire::PlayerAppearance {
        account: entry.account,
        character: character.id,
        name: character.name,
        job: character.job,
        hair: character.hair,
        hair_color: character.hair_color,
        sex: character.sex,
    };
    let (output, packets) = mpsc::sync_channel::<Vec<u8>>(128);
    let Some(session) = sessions.admit(appearance, (SPAWN_X, SPAWN_Y), SPAWN_DIRECTION, output)?
    else {
        stream.write_all(&wire::map_entry_refused())?;
        return listener::drain(stream);
    };
    // Read after admission. The prior connection holds its lease until its save finishes.
    let spawn = session.restore_position(db.character_position(
        entry.account,
        entry.character,
        START_MAP,
    )?)?;
    stream.write_all(&wire::map_account_ack(entry.account))?;
    println!(
        "Accepted map handoff for character {} to {}",
        entry.character, START_MAP
    );
    stream.write_all(&wire::map_accepted(
        server_tick(started),
        spawn.0,
        spawn.1,
        SPAWN_DIRECTION,
    ))?;
    // Every post-admission packet goes through one writer. The map only enqueues.
    let mut writer = stream.try_clone()?;
    thread::Builder::new()
        .name("map-writer".into())
        .spawn(move || {
            for packet in packets {
                if writer.write_all(&packet).is_err() {
                    break;
                }
            }
            let _ = writer.shutdown(Shutdown::Both);
        })?;
    let connection = run_session(stream, started, &session, entry.character, spawn);
    // This runs on the connection thread, including when the peer resets the socket.
    if let Some(tile) = session.finish()? {
        db.save_character_position(entry.account, entry.character, START_MAP, tile)?;
    }
    connection
}

fn run_session(
    mut stream: TcpStream,
    started: Instant,
    session: &Session<'_>,
    character: u32,
    spawn: Tile,
) -> Result<()> {
    let load_deadline = Instant::now() + LOAD_TIMEOUT;
    let mut loaded = false;
    loop {
        let timeout = if loaded {
            LOAD_TIMEOUT
        } else {
            match load_deadline.checked_duration_since(Instant::now()) {
                Some(time) if !time.is_zero() => time,
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "map load timed out",
                    )
                    .into())
                }
            }
        };
        stream.set_read_timeout(Some(timeout))?;
        match wire::read_map_request(&mut stream)? {
            wire::MapRequest::Loaded => {
                if session.loaded()? {
                    loaded = true;
                    println!(
                        "Character {} loaded {} at ({}, {})",
                        character, START_MAP, spawn.0, spawn.1
                    );
                }
            }
            wire::MapRequest::ConfirmLoad if loaded => session.send(wire::MAP_LOAD_CONFIRMED)?,
            wire::MapRequest::ConfirmLoad
            | wire::MapRequest::RemainingPlaytime
            | wire::MapRequest::CashShopCatalog => {}
            wire::MapRequest::SyncTime => session.send(wire::map_time(server_tick(started)))?,
            wire::MapRequest::Ping => session.send(wire::MAP_PONG)?,
            wire::MapRequest::Move(x, y) => {
                let destination = (x, y);
                if let Some(position) = session.begin_walk(destination)? {
                    println!(
                        "Character {} walking from {:?} to {:?}",
                        character, position, destination
                    );
                }
            }
        }
    }
}

fn server_tick(started: Instant) -> u32 {
    started.elapsed().as_millis() as u32
}
