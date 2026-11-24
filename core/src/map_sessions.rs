//! The map loop owns live state. Connections send commands through a bounded queue.
use std::collections::{hash_map::Entry, HashMap};
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use crate::terrain::{Terrain, Tile};

const COMMAND_CAPACITY: usize = 128;
const TICK_INTERVAL: Duration = Duration::from_millis(25);

pub struct MapSessions {
    commands: SyncSender<Command>,
}

enum Command {
    Admit(u32, Tile, SyncSender<bool>),
    Loaded(u32, SyncSender<bool>),
    Walk(u32, Tile, SyncSender<Option<Tile>>),
    Disconnect(u32),
    #[cfg(test)]
    Position(u32, SyncSender<Option<Tile>>),
}

struct Player {
    loaded: bool,
    tile: Tile,
    walk: Option<(Tile, Instant)>,
}

impl Player {
    fn advance(&mut self, now: Instant) {
        if let Some((target, arrival)) = self.walk {
            if now >= arrival {
                self.tile = target;
                self.walk = None;
            }
        }
    }

    fn begin_walk(
        &mut self,
        terrain: Option<&Terrain>,
        now: Instant,
        target: Tile,
    ) -> Option<Tile> {
        self.advance(now);
        if !self.loaded || self.walk.is_some() {
            return None;
        }
        let duration = terrain?.walk_duration(self.tile, target)?;
        self.walk = Some((target, now + duration));
        Some(self.tile)
    }
}

impl MapSessions {
    pub fn start(terrain: Option<Terrain>) -> io::Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        thread::Builder::new()
            .name("map-loop".into())
            .spawn(move || run(receiver, terrain))?;
        Ok(Self { commands })
    }

    pub fn admit(&self, character: u32, spawn: Tile) -> io::Result<Option<Session<'_>>> {
        let admitted = self.request(|reply| Command::Admit(character, spawn, reply))?;
        Ok(admitted.then(|| Session {
            map: self,
            character,
        }))
    }

    fn request<T>(&self, command: impl FnOnce(SyncSender<T>) -> Command) -> io::Result<T> {
        let (reply, result) = mpsc::sync_channel(1);
        self.send(command(reply))?;
        result.recv().map_err(|_| map_stopped())
    }

    fn send(&self, command: Command) -> io::Result<()> {
        self.commands.send(command).map_err(|_| map_stopped())
    }
}

fn run(commands: Receiver<Command>, terrain: Option<Terrain>) {
    let mut players = HashMap::<u32, Player>::new();
    let mut next_tick = Instant::now() + TICK_INTERVAL;
    loop {
        let now = Instant::now();
        if now >= next_tick {
            for player in players.values_mut() {
                player.advance(now);
            }
            next_tick = now + TICK_INTERVAL;
        }
        let command = match commands.recv_timeout(next_tick.saturating_duration_since(now)) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match command {
            Command::Admit(character, spawn, reply) => {
                if let Entry::Vacant(entry) = players.entry(character) {
                    entry.insert(Player {
                        loaded: false,
                        tile: spawn,
                        walk: None,
                    });
                    let _ = reply.send(true);
                } else {
                    let _ = reply.send(false);
                }
            }
            Command::Loaded(character, reply) => {
                let first = players
                    .get_mut(&character)
                    .is_some_and(|player| !std::mem::replace(&mut player.loaded, true));
                let _ = reply.send(first);
            }
            Command::Walk(character, target, reply) => {
                let source = players
                    .get_mut(&character)
                    .and_then(|player| player.begin_walk(terrain.as_ref(), Instant::now(), target));
                let _ = reply.send(source);
            }
            Command::Disconnect(character) => {
                players.remove(&character);
            }
            #[cfg(test)]
            Command::Position(character, reply) => {
                let tile = players
                    .get(&character)
                    .filter(|player| player.loaded)
                    .map(|player| player.tile);
                let _ = reply.send(tile);
            }
        }
    }
}

fn map_stopped() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "map loop stopped")
}

/// A connection's exclusive lease. Its disconnect queues behind its earlier commands.
pub struct Session<'a> {
    map: &'a MapSessions,
    character: u32,
}

impl Session<'_> {
    pub fn loaded(&self) -> io::Result<bool> {
        self.map
            .request(|reply| Command::Loaded(self.character, reply))
    }

    pub fn begin_walk(&self, target: Tile) -> io::Result<Option<Tile>> {
        self.map
            .request(|reply| Command::Walk(self.character, target, reply))
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        let _ = self.map.send(Command::Disconnect(self.character));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_loop_rejects_duplicate_and_releases_on_disconnect() {
        let map = MapSessions::start(None).unwrap();
        let first = map.admit(7, (1, 2)).unwrap().unwrap();
        assert!(map.admit(7, (3, 4)).unwrap().is_none());
        assert_eq!(
            map.request(|reply| Command::Position(7, reply)).unwrap(),
            None
        );
        assert!(first.loaded().unwrap());
        assert!(!first.loaded().unwrap());
        assert_eq!(
            map.request(|reply| Command::Position(7, reply)).unwrap(),
            Some((1, 2))
        );
        drop(first);
        let second = map.admit(7, (3, 4)).unwrap().unwrap();
        assert!(second.loaded().unwrap());
        assert_eq!(
            map.request(|reply| Command::Position(7, reply)).unwrap(),
            Some((3, 4))
        );
    }

    #[test]
    fn map_tick_settles_arrival_without_a_second_move() {
        let now = Instant::now();
        let arrival = now + Duration::from_millis(150);
        let mut player = Player {
            loaded: true,
            tile: (1, 2),
            walk: Some(((2, 2), arrival)),
        };
        player.advance(now);
        assert_eq!(player.tile, (1, 2));
        player.advance(arrival);
        assert_eq!(player.tile, (2, 2));
        assert!(player.walk.is_none());
    }
}
