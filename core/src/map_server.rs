use std::io::Write;
use std::net::{SocketAddrV4, TcpStream};
use std::time::{Duration, Instant};

use rust_athena_db::Database;

use crate::{listener, protocol::v20260219 as wire, Result};

pub const START_MAP: &str = "prontera.gat";
const SPAWN_X: u16 = 156;
const SPAWN_Y: u16 = 191;
const SPAWN_DIRECTION: u8 = 4;
const LOAD_TIMEOUT: Duration = Duration::from_secs(90);

pub fn serve(db: Database, bind: SocketAddrV4) -> Result<()> {
    let started = Instant::now();
    listener::serve(bind, Duration::from_secs(10), move |stream| {
        handle(&db, stream, started)
    })
}

fn handle(db: &Database, mut stream: TcpStream, started: Instant) -> Result<()> {
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

    stream.write_all(&wire::map_account_ack(entry.account))?;
    println!(
        "Accepted map handoff for character {} to {}",
        entry.character, START_MAP
    );
    stream.write_all(&wire::map_accepted(
        server_tick(started),
        SPAWN_X,
        SPAWN_Y,
        SPAWN_DIRECTION,
    ))?;
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
                stream.write_all(&wire::MAP_LOAD_CONFIRMED)?;
                if !loaded {
                    loaded = true;
                    println!(
                        "Character {} loaded {} at ({}, {})",
                        entry.character, START_MAP, SPAWN_X, SPAWN_Y
                    );
                }
            }
            wire::MapRequest::ConfirmLoad if loaded => {
                stream.write_all(&wire::MAP_LOAD_CONFIRMED)?
            }
            wire::MapRequest::ConfirmLoad => {}
            wire::MapRequest::SyncTime => {
                stream.write_all(&wire::map_time(server_tick(started)))?
            }
            wire::MapRequest::Ping => stream.write_all(&wire::MAP_PONG)?,
        }
    }
}

fn server_tick(started: Instant) -> u32 {
    started.elapsed().as_millis() as u32
}
