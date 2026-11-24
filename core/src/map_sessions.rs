//! A map process owns its live sessions. A connection holds a lease on one session.
use std::cmp::Ordering::{Equal, Greater, Less};
use std::collections::{hash_map::Entry, HashMap, HashSet, VecDeque};
use std::sync::{mpsc::SyncSender, Mutex};
use std::time::{Duration, Instant};

use crate::protocol::v20260219 as wire;
use crate::terrain::{Terrain, Tile, WALK_MILLIS};

const SIGHT: u16 = 14;

pub struct MapSessions {
    started: Instant,
    state: Mutex<MapState>,
}

#[derive(Default)]
struct MapState {
    players: HashMap<u32, Player>,
    // Each sorted pair represents mutual visibility.
    visible: HashSet<(u32, u32)>,
}

struct Player {
    appearance: wire::PlayerAppearance,
    output: Option<SyncSender<Vec<u8>>>,
    loaded: bool,
    tile: Tile,
    direction: u8,
    steps: VecDeque<(Instant, Tile)>,
}

impl Player {
    fn advance(&mut self, now: Instant) -> bool {
        let before = self.tile;
        while let Some(&(arrival, tile)) = self.steps.front() {
            if now < arrival {
                break;
            }
            self.steps.pop_front();
            self.direction = match (tile.0.cmp(&self.tile.0), tile.1.cmp(&self.tile.1)) {
                (Equal, Greater) => 0,
                (Less, Greater) => 1,
                (Less, Equal) => 2,
                (Less, Less) => 3,
                (Equal, Less) => 4,
                (Greater, Less) => 5,
                (Greater, Equal) => 6,
                (Greater, Greater) => 7,
                (Equal, Equal) => self.direction,
            };
            self.tile = tile;
        }
        before != self.tile
    }

    fn entry_packet(&self, tick: u32) -> Vec<u8> {
        let walk = self
            .steps
            .back()
            .map(|&(_, destination)| (tick, destination));
        wire::visible_player(&self.appearance, self.tile, self.direction, walk)
    }

    fn send(&mut self, packet: Vec<u8>) {
        if let Some(output) = &self.output {
            if output.try_send(packet).is_err() {
                // Closing the queue makes the writer shut down a slow or lost peer.
                self.output = None;
            }
        }
    }
}

impl MapState {
    fn advance(&mut self, now: Instant, tick: u32) {
        let mut changed = false;
        for player in self.players.values_mut() {
            changed |= player.advance(now);
        }
        if changed {
            self.refresh_visibility(tick);
        }
    }

    fn refresh_visibility(&mut self, tick: u32) {
        let active: Vec<_> = self
            .players
            .iter()
            .filter(|(_, player)| player.loaded)
            .collect();
        let mut visible = HashSet::new();
        for (index, (&left_id, left)) in active.iter().copied().enumerate() {
            for (&right_id, right) in active[index + 1..].iter().copied() {
                if left
                    .tile
                    .0
                    .abs_diff(right.tile.0)
                    .max(left.tile.1.abs_diff(right.tile.1))
                    <= SIGHT
                {
                    visible.insert((left_id.min(right_id), left_id.max(right_id)));
                }
            }
        }
        let previous = std::mem::replace(&mut self.visible, visible);
        for &(left, right) in self.visible.difference(&previous) {
            let left_entry = self.players[&left].entry_packet(tick);
            let right_entry = self.players[&right].entry_packet(tick);
            self.players.get_mut(&left).unwrap().send(right_entry);
            self.players.get_mut(&right).unwrap().send(left_entry);
        }
        for &(left, right) in previous.difference(&self.visible) {
            if let Some(player) = self.players.get_mut(&left) {
                player.send(wire::player_vanished(right));
            }
            if let Some(player) = self.players.get_mut(&right) {
                player.send(wire::player_vanished(left));
            }
        }
    }
}

impl MapSessions {
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            state: Mutex::default(),
        }
    }

    fn tick(&self, now: Instant) -> u32 {
        now.duration_since(self.started).as_millis() as u32
    }

    pub fn advance(&self, now: Instant) {
        self.state.lock().unwrap().advance(now, self.tick(now));
    }

    pub fn admit(
        &self,
        appearance: wire::PlayerAppearance,
        spawn: Tile,
        direction: u8,
        output: SyncSender<Vec<u8>>,
    ) -> Option<Session<'_>> {
        let account = appearance.account;
        let mut state = self.state.lock().unwrap();
        let Entry::Vacant(entry) = state.players.entry(account) else {
            return None;
        };
        entry.insert(Player {
            appearance,
            output: Some(output),
            loaded: false,
            tile: spawn,
            direction,
            steps: VecDeque::new(),
        });
        Some(Session { map: self, account })
    }
}

/// A live connection's exclusive lease. Its account ID is also its wire actor ID.
pub struct Session<'a> {
    map: &'a MapSessions,
    account: u32,
}

impl Session<'_> {
    pub fn send(&self, packet: impl Into<Vec<u8>>) {
        self.map
            .state
            .lock()
            .unwrap()
            .players
            .get_mut(&self.account)
            .unwrap()
            .send(packet.into());
    }

    pub fn loaded(&self, now: Instant) -> bool {
        let mut state = self.map.state.lock().unwrap();
        let tick = self.map.tick(now);
        state.advance(now, tick);
        let player = state.players.get_mut(&self.account).unwrap();
        if std::mem::replace(&mut player.loaded, true) {
            return false;
        }
        state.refresh_visibility(tick);
        true
    }

    pub fn begin_walk(&self, terrain: &Terrain, now: Instant, target: Tile) -> Option<Tile> {
        let mut state = self.map.state.lock().unwrap();
        let tick = self.map.tick(now);
        state.advance(now, tick);
        let player = state.players.get_mut(&self.account).unwrap();
        if !player.loaded || !player.steps.is_empty() {
            return None;
        }
        let source = player.tile;
        let route = terrain.route(source, target)?;
        let mut previous = source;
        let mut arrival = now;
        for tile in route {
            let diagonal = tile.0 != previous.0 && tile.1 != previous.1;
            arrival += Duration::from_millis(if diagonal {
                WALK_MILLIS * 14 / 10
            } else {
                WALK_MILLIS
            });
            player.steps.push_back((arrival, tile));
            previous = tile;
        }
        player.send(wire::player_move(tick, source, target).to_vec());

        let packet = player.entry_packet(tick);
        let MapState { visible, players } = &mut *state;
        for &(left, right) in visible.iter() {
            let viewer = if left == self.account {
                right
            } else if right == self.account {
                left
            } else {
                continue;
            };
            players.get_mut(&viewer).unwrap().send(packet.clone());
        }
        Some(source)
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        let mut state = self.map.state.lock().unwrap();
        state.players.remove(&self.account);
        state.refresh_visibility(self.map.tick(Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{self, Receiver};

    fn open(map: &MapSessions, account: u32, tile: Tile) -> (Session<'_>, Receiver<Vec<u8>>) {
        let (sender, receiver) = mpsc::sync_channel(8);
        let session = map
            .admit(
                wire::PlayerAppearance {
                    account,
                    character: account + 100,
                    name: format!("Player{}", account),
                    job: 4218,
                    hair: 3,
                    hair_color: 2,
                    sex: 1,
                },
                tile,
                4,
                sender,
            )
            .unwrap();
        (session, receiver)
    }

    fn packet(receiver: &Receiver<Vec<u8>>, opcode: u16, account: u32, offset: usize) -> Vec<u8> {
        let packet = receiver.try_recv().unwrap();
        assert_eq!(&packet[..2], &opcode.to_le_bytes());
        assert_eq!(&packet[offset..offset + 4], &account.to_le_bytes());
        packet
    }

    #[test]
    fn loaded_players_enter_leave_and_reenter_mutual_visibility() {
        let now = Instant::now();
        let map = MapSessions::new(now);
        let (left, left_rx) = open(&map, 7, (156, 191));
        let (right, right_rx) = open(&map, 8, (170, 191));
        assert!(left.loaded(now));
        assert!(left_rx.try_recv().is_err());
        assert!(right.loaded(now));
        let entry = packet(&left_rx, 0x09ff, 8, 5);
        assert_eq!(entry.len(), 108);
        assert_eq!(&entry[2..4], &108u16.to_le_bytes());
        assert_eq!(&entry[9..13], &108u32.to_le_bytes());
        assert_eq!(&entry[0x54..0x5b], b"Player8");
        packet(&right_rx, 0x09ff, 7, 5);

        let arrival = now + Duration::from_millis(WALK_MILLIS);
        map.state
            .lock()
            .unwrap()
            .players
            .get_mut(&8)
            .unwrap()
            .steps
            .push_back((arrival, (171, 191)));
        map.advance(arrival);
        packet(&left_rx, 0x0080, 8, 2);
        packet(&right_rx, 0x0080, 7, 2);

        let returned = arrival + Duration::from_millis(WALK_MILLIS);
        map.state
            .lock()
            .unwrap()
            .players
            .get_mut(&8)
            .unwrap()
            .steps
            .push_back((returned, (170, 191)));
        map.advance(returned);
        packet(&left_rx, 0x09ff, 8, 5);
        packet(&right_rx, 0x09ff, 7, 5);

        drop(right);
        packet(&left_rx, 0x0080, 8, 2);
        let (reconnected, reconnected_rx) = open(&map, 8, (156, 191));
        assert!(reconnected.loaded(returned));
        packet(&left_rx, 0x09ff, 8, 5);
        packet(&reconnected_rx, 0x09ff, 7, 5);
        assert!(left_rx.try_recv().is_err());
        assert!(!reconnected.loaded(returned));
    }

    #[test]
    fn duplicate_account_cannot_displace_an_existing_actor() {
        let map = MapSessions::new(Instant::now());
        let (first, _) = open(&map, 7, (1, 2));
        let (output, _) = mpsc::sync_channel(8);
        assert!(map
            .admit(
                wire::PlayerAppearance {
                    account: 7,
                    character: 109,
                    name: "Duplicate".into(),
                    job: 0,
                    hair: 0,
                    hair_color: 0,
                    sex: 1,
                },
                (3, 4),
                4,
                output
            )
            .is_none());
        assert_eq!(map.state.lock().unwrap().players[&7].tile, (1, 2));
        drop(first);
        let (_second, _) = open(&map, 7, (3, 4));
        assert_eq!(map.state.lock().unwrap().players[&7].tile, (3, 4));
    }
}
