//! A map process owns its live sessions. A connection holds a lease on one session.
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Mutex;
use std::time::Instant;

use crate::terrain::{Terrain, Tile};

#[derive(Default)]
pub struct MapSessions {
    players: Mutex<HashMap<u32, Player>>,
}

struct Player {
    loaded: bool,
    tile: Tile,
    walk: Option<(Tile, Instant)>,
}

impl Player {
    fn position(&mut self, now: Instant) -> Tile {
        if let Some((target, arrival)) = self.walk {
            if now >= arrival {
                self.tile = target;
                self.walk = None;
            }
        }
        self.tile
    }
}

impl MapSessions {
    pub fn active_position(&self, character: u32, now: Instant) -> Option<Tile> {
        let mut players = self.players.lock().unwrap();
        let player = players.get_mut(&character)?;
        player.loaded.then(|| player.position(now))
    }

    pub fn admit(&self, character: u32, spawn: Tile) -> Option<Session<'_>> {
        let mut players = self.players.lock().unwrap();
        let Entry::Vacant(entry) = players.entry(character) else {
            return None;
        };
        entry.insert(Player {
            loaded: false,
            tile: spawn,
            walk: None,
        });
        Some(Session {
            map: self,
            character,
        })
    }
}

/// A live connection's exclusive lease. Reconnects can claim the character
/// after this lease is dropped, including on a failed write or read timeout.
pub struct Session<'a> {
    map: &'a MapSessions,
    character: u32,
}

impl Session<'_> {
    pub fn loaded(&self) -> bool {
        let mut players = self.map.players.lock().unwrap();
        let player = players.get_mut(&self.character).unwrap();
        !std::mem::replace(&mut player.loaded, true)
    }

    pub fn begin_walk(&self, terrain: &Terrain, now: Instant, target: Tile) -> Option<Tile> {
        let mut players = self.map.players.lock().unwrap();
        let player = players.get_mut(&self.character).unwrap();
        let source = player.position(now);
        if !player.loaded || player.walk.is_some() {
            return None;
        }
        let duration = terrain.walk_duration(source, target)?;
        player.walk = Some((target, now + duration));
        Some(source)
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        self.map.players.lock().unwrap().remove(&self.character);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn lease_rejects_duplicate_connections_and_releases_character_on_drop() {
        let map = MapSessions::default();
        let first = map.admit(7, (1, 2)).unwrap();
        assert!(map.admit(7, (3, 4)).is_none());
        assert_eq!(map.active_position(7, Instant::now()), None);
        assert!(first.loaded());
        assert!(!first.loaded());
        assert_eq!(map.active_position(7, Instant::now()), Some((1, 2)));
        drop(first);
        let _second = map.admit(7, (3, 4)).unwrap();
        assert_eq!(map.players.lock().unwrap()[&7].tile, (3, 4));
    }

    #[test]
    fn position_settles_on_query_after_arrival_without_another_move() {
        let now = Instant::now();
        let arrival = now + Duration::from_millis(150);
        let mut player = Player {
            loaded: true,
            tile: (1, 2),
            walk: Some(((2, 2), arrival)),
        };
        assert_eq!(player.position(now), (1, 2));
        assert_eq!(player.position(arrival), (2, 2));
        assert!(player.walk.is_none());
    }
}
