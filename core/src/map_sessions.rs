//! A map process owns its live sessions. A connection holds a lease on one session.
use std::cmp::Ordering::{Equal, Greater, Less};
use std::collections::{hash_map::Entry, HashMap, HashSet, VecDeque};
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use crate::protocol::v20260219 as wire;
use crate::terrain::{Terrain, Tile, WALK_MILLIS};

const SIGHT: u16 = 14;
const COMMAND_CAPACITY: usize = 128;
const TICK_INTERVAL: Duration = Duration::from_millis(25);

pub struct MapSessions {
    commands: SyncSender<Command>,
}

enum Command {
    Admit(Player, SyncSender<bool>),
    Spawn(u32, Option<Tile>, SyncSender<Tile>),
    Loaded(u32, SyncSender<bool>),
    Finish(u32, SyncSender<Option<Tile>>),
    Walk(u32, Tile, SyncSender<Option<Tile>>),
    Face(u32, u16, u8),
    Reply(u32, Vec<u8>),
    Disconnect(u32),
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
    head_direction: u16,
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
        wire::visible_player(
            &self.appearance,
            self.tile,
            self.direction,
            self.head_direction,
            walk,
        )
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
    pub fn start(started: Instant, terrain: Option<Terrain>) -> io::Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(COMMAND_CAPACITY);
        thread::Builder::new()
            .name("map-loop".into())
            .spawn(move || run(receiver, started, terrain))?;
        Ok(Self { commands })
    }

    pub fn admit(
        &self,
        appearance: wire::PlayerAppearance,
        spawn: Tile,
        direction: u8,
        output: SyncSender<Vec<u8>>,
    ) -> io::Result<Option<Session<'_>>> {
        let account = appearance.account;
        let player = Player {
            appearance,
            output: Some(output),
            loaded: false,
            tile: spawn,
            direction,
            head_direction: 0,
            steps: VecDeque::new(),
        };
        let admitted = self.request(|reply| Command::Admit(player, reply))?;
        Ok(admitted.then(|| Session { map: self, account }))
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

fn run(commands: Receiver<Command>, started: Instant, terrain: Option<Terrain>) {
    let tick = |now: Instant| now.duration_since(started).as_millis() as u32;
    let mut state = MapState::default();
    let mut next_tick = Instant::now() + TICK_INTERVAL;
    loop {
        let now = Instant::now();
        if now >= next_tick {
            state.advance(now, tick(now));
            next_tick = now + TICK_INTERVAL;
        }
        let command = match commands.recv_timeout(next_tick.saturating_duration_since(now)) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let now = Instant::now();
        match command {
            Command::Admit(player, reply) => {
                let _ = reply.send(state.admit(player));
            }
            Command::Spawn(account, saved, reply) => {
                let player = state.players.get_mut(&account).unwrap();
                if let Some(tile) = saved.filter(|&tile| valid_spawn(terrain.as_ref(), tile)) {
                    player.tile = tile;
                }
                let _ = reply.send(player.tile);
            }
            Command::Finish(account, reply) => {
                let _ = reply.send(state.finish(account, now, tick(now)));
            }
            Command::Loaded(account, reply) => {
                let _ = reply.send(state.loaded(account, now, tick(now)));
            }
            Command::Walk(account, destination, reply) => {
                let source = terrain.as_ref().and_then(|terrain| {
                    state.begin_walk(account, terrain, now, tick(now), destination)
                });
                let _ = reply.send(source);
            }
            Command::Face(account, head, body) => {
                state.advance(now, tick(now));
                state.face(account, head, body);
            }
            Command::Reply(account, packet) => {
                if let Some(player) = state.players.get_mut(&account) {
                    player.send(packet);
                }
            }
            Command::Disconnect(account) => {
                state.players.remove(&account);
                state.refresh_visibility(tick(now));
            }
        }
    }
}

fn valid_spawn(terrain: Option<&Terrain>, (x, y): Tile) -> bool {
    x < 1024 && y < 1024 && terrain.is_none_or(|terrain| terrain.walkable((x, y)))
}

fn map_stopped() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "map loop stopped")
}

/// A live connection's exclusive lease. Its account ID is also its wire actor ID.
pub struct Session<'a> {
    map: &'a MapSessions,
    account: u32,
}

impl Session<'_> {
    pub fn send(&self, packet: impl Into<Vec<u8>>) -> io::Result<()> {
        self.map.send(Command::Reply(self.account, packet.into()))
    }

    pub fn restore_position(&self, saved: Option<Tile>) -> io::Result<Tile> {
        self.map
            .request(|reply| Command::Spawn(self.account, saved, reply))
    }

    /// Remove visibility now, but keep the account reserved until the caller saves.
    pub fn finish(&self) -> io::Result<Option<Tile>> {
        self.map
            .request(|reply| Command::Finish(self.account, reply))
    }

    pub fn loaded(&self) -> io::Result<bool> {
        self.map
            .request(|reply| Command::Loaded(self.account, reply))
    }

    pub fn face(&self, head: u16, body: u8) -> io::Result<()> {
        self.map.send(Command::Face(self.account, head, body))
    }

    pub fn begin_walk(&self, target: Tile) -> io::Result<Option<Tile>> {
        self.map
            .request(|reply| Command::Walk(self.account, target, reply))
    }
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        let _ = self.map.send(Command::Disconnect(self.account));
    }
}

impl MapState {
    fn admit(&mut self, player: Player) -> bool {
        let Entry::Vacant(entry) = self.players.entry(player.appearance.account) else {
            return false;
        };
        entry.insert(player);
        true
    }

    fn finish(&mut self, account: u32, now: Instant, tick: u32) -> Option<Tile> {
        let player = self.players.get_mut(&account)?;
        if !std::mem::replace(&mut player.loaded, false) {
            return None;
        }
        player.advance(now);
        player.steps.clear();
        let tile = player.tile;
        self.refresh_visibility(tick);
        Some(tile)
    }

    fn loaded(&mut self, account: u32, now: Instant, tick: u32) -> bool {
        self.advance(now, tick);
        let Some(player) = self.players.get_mut(&account) else {
            return false;
        };
        player.send(wire::MAP_LOAD_CONFIRMED.to_vec());
        if std::mem::replace(&mut player.loaded, true) {
            return false;
        }
        self.refresh_visibility(tick);
        true
    }

    fn begin_walk(
        &mut self,
        account: u32,
        terrain: &Terrain,
        now: Instant,
        tick: u32,
        target: Tile,
    ) -> Option<Tile> {
        self.advance(now, tick);
        let player = self.players.get_mut(&account)?;
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
        self.send_to_observers(account, packet);
        Some(source)
    }

    fn face(&mut self, account: u32, head: u16, body: u8) {
        let Some(player) = self.players.get_mut(&account) else {
            return;
        };
        if !player.loaded || head > 2 || body > 7 {
            return;
        }
        player.head_direction = head;
        player.direction = body;
        self.send_to_observers(account, wire::player_facing(account, head, body).to_vec());
    }

    fn send_to_observers(&mut self, account: u32, packet: Vec<u8>) {
        let MapState { visible, players } = self;
        for &(left, right) in visible.iter() {
            let viewer = if left == account {
                right
            } else if right == account {
                left
            } else {
                continue;
            };
            players.get_mut(&viewer).unwrap().send(packet.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{self, Receiver};

    fn open(map: &mut MapState, account: u32, tile: Tile) -> Receiver<Vec<u8>> {
        let (sender, receiver) = mpsc::sync_channel(8);
        assert!(map.admit(Player {
            appearance: wire::PlayerAppearance {
                account,
                character: account + 100,
                name: format!("Player{}", account),
                job: 4218,
                hair: 3,
                hair_color: 2,
                sex: 1
            },
            output: Some(sender),
            loaded: false,
            tile,
            direction: 4,
            head_direction: 0,
            steps: VecDeque::new(),
        }));
        receiver
    }

    fn packet(receiver: &Receiver<Vec<u8>>, opcode: u16, account: u32, offset: usize) -> Vec<u8> {
        let packet = receiver.try_recv().unwrap();
        assert_eq!(&packet[..2], &opcode.to_le_bytes());
        assert_eq!(&packet[offset..offset + 4], &account.to_le_bytes());
        packet
    }

    #[test]
    fn facing_is_sent_only_to_loaded_visible_peers_and_retained_for_new_ones() {
        let now = Instant::now();
        let mut map = MapState::default();
        let actor = open(&mut map, 7, (156, 191));
        let peer = open(&mut map, 8, (157, 191));
        let far = open(&mut map, 9, (180, 191));
        assert!(map.loaded(8, now, 0));
        assert!(map.loaded(9, now, 0));
        assert_eq!(peer.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        assert_eq!(far.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        map.face(7, 2, 7);
        assert!(peer.try_recv().is_err());
        assert!(map.loaded(7, now, 0));
        assert_eq!(actor.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        packet(&actor, 0x09ff, 8, 5);
        packet(&peer, 0x09ff, 7, 5);
        map.face(7, 3, 0);
        map.face(7, 0, 8);
        assert!(peer.try_recv().is_err());
        map.face(7, 2, 7);
        assert_eq!(packet(&peer, 0x009c, 7, 2)[6..], [2, 0, 7]);
        assert!(actor.try_recv().is_err());
        assert!(far.try_recv().is_err());
        map.players.get_mut(&9).unwrap().tile = (158, 191);
        map.refresh_visibility(1);
        let entry = (0..2)
            .map(|_| far.try_recv().unwrap())
            .find(|packet| packet[5..9] == 7u32.to_le_bytes())
            .unwrap();
        assert_eq!(&entry[0x2d..0x2f], &[2, 0]);
        assert_eq!(entry[0x41] & 15, 7);
    }

    #[test]
    fn finishing_saves_only_reached_tiles_and_retires_visibility() {
        let now = Instant::now();
        let mut map = MapState::default();
        let left = open(&mut map, 7, (156, 191));
        let right = open(&mut map, 8, (156, 191));
        assert_eq!(map.finish(8, now, 0), None);
        assert!(map.loaded(7, now, 0));
        assert!(map.loaded(8, now, 0));
        assert_eq!(left.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        assert_eq!(right.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        packet(&left, 0x09ff, 8, 5);
        packet(&right, 0x09ff, 7, 5);
        let player = map.players.get_mut(&8).unwrap();
        player.steps.extend([
            (now + Duration::from_millis(150), (157, 191)),
            (now + Duration::from_millis(300), (158, 191)),
        ]);
        assert_eq!(
            map.finish(8, now + Duration::from_millis(200), 200),
            Some((157, 191))
        );
        packet(&left, 0x0080, 8, 2);
        assert!(map.players.contains_key(&8));
    }

    #[test]
    fn loaded_players_enter_leave_and_reenter_mutual_visibility() {
        let now = Instant::now();
        let mut map = MapState::default();
        let left_rx = open(&mut map, 7, (156, 191));
        let right_rx = open(&mut map, 8, (170, 191));
        assert!(map.loaded(7, now, 0));
        assert_eq!(left_rx.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        assert!(left_rx.try_recv().is_err());
        assert!(map.loaded(8, now, 0));
        assert_eq!(right_rx.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        let entry = packet(&left_rx, 0x09ff, 8, 5);
        assert_eq!(entry.len(), 108);
        assert_eq!(&entry[2..4], &108u16.to_le_bytes());
        assert_eq!(&entry[9..13], &108u32.to_le_bytes());
        assert_eq!(&entry[0x54..0x5b], b"Player8");
        packet(&right_rx, 0x09ff, 7, 5);

        let arrival = now + Duration::from_millis(WALK_MILLIS);
        map.players
            .get_mut(&8)
            .unwrap()
            .steps
            .push_back((arrival, (171, 191)));
        map.advance(arrival, 150);
        packet(&left_rx, 0x0080, 8, 2);
        packet(&right_rx, 0x0080, 7, 2);

        let returned = arrival + Duration::from_millis(WALK_MILLIS);
        map.players
            .get_mut(&8)
            .unwrap()
            .steps
            .push_back((returned, (170, 191)));
        map.advance(returned, 300);
        packet(&left_rx, 0x09ff, 8, 5);
        packet(&right_rx, 0x09ff, 7, 5);

        map.players.remove(&8);
        map.refresh_visibility(300);
        packet(&left_rx, 0x0080, 8, 2);
        let reconnected_rx = open(&mut map, 8, (156, 191));
        assert!(map.loaded(8, returned, 300));
        assert_eq!(reconnected_rx.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        packet(&left_rx, 0x09ff, 8, 5);
        packet(&reconnected_rx, 0x09ff, 7, 5);
        assert!(left_rx.try_recv().is_err());
        assert!(!map.loaded(8, returned, 300));
    }

    #[test]
    fn duplicate_account_cannot_displace_an_existing_actor() {
        let map = MapSessions::start(Instant::now(), None).unwrap();
        let admit = |character| {
            let (sender, receiver) = mpsc::sync_channel(8);
            let session = map
                .admit(
                    wire::PlayerAppearance {
                        account: 7,
                        character,
                        name: "Player".into(),
                        job: 0,
                        hair: 0,
                        hair_color: 0,
                        sex: 1,
                    },
                    (1, 2),
                    4,
                    sender,
                )
                .unwrap();
            (session, receiver)
        };
        let (first, first_rx) = admit(107);
        let first = first.unwrap();
        assert!(admit(109).0.is_none());
        assert!(first.loaded().unwrap());
        assert_eq!(first_rx.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
        assert_eq!(first.finish().unwrap(), Some((1, 2)));
        assert!(
            admit(109).0.is_none(),
            "the finishing lease keeps its reservation during the save"
        );
        drop(first);
        let (next, next_rx) = admit(109);
        let next = next.unwrap();
        assert_eq!(next.restore_position(Some((1024, 3))).unwrap(), (1, 2));
        assert_eq!(next.restore_position(Some((4, 3))).unwrap(), (4, 3));
        assert!(next.loaded().unwrap());
        assert_eq!(next_rx.try_recv().unwrap(), wire::MAP_LOAD_CONFIRMED);
    }
}
