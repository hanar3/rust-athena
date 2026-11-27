//! Client wire format used by the 2026-02-19 login and character listeners.
use std::convert::TryInto;
use std::io::{self, Read};
use std::net::SocketAddrV4;

pub type CharacterRecord = [u8; 175];

/// Fields populated by the current roster. Remaining wire fields are zero.
pub struct CharacterSummary<'a> {
    pub id: u32,
    pub job_level: i32,
    pub hp: i64,
    pub max_hp: i64,
    pub sp: i64,
    pub max_sp: i64,
    pub walk_speed: i16,
    pub base_level: i16,
    pub name: &'a [u8],
    pub slot: u8,
    pub job: i16,
    pub hair: i16,
    pub hair_color: i16,
    pub map: &'a [u8],
    pub sex: u8,
}

impl CharacterSummary<'_> {
    pub fn encode(&self) -> io::Result<CharacterRecord> {
        let mut record = [0; 175];
        record[..4].copy_from_slice(&self.id.to_le_bytes());
        record[0x18..0x1c].copy_from_slice(&self.job_level.to_le_bytes());
        record[0x32..0x3a].copy_from_slice(&self.hp.to_le_bytes());
        record[0x3a..0x42].copy_from_slice(&self.max_hp.to_le_bytes());
        record[0x42..0x4a].copy_from_slice(&self.sp.to_le_bytes());
        record[0x4a..0x52].copy_from_slice(&self.max_sp.to_le_bytes());
        record[0x52..0x54].copy_from_slice(&self.walk_speed.to_le_bytes());
        record[0x5c..0x5e].copy_from_slice(&self.base_level.to_le_bytes());
        write_field(&mut record[0x6c..0x84], self.name, "character name")?;
        record[0x54..0x56].copy_from_slice(&self.job.to_le_bytes());
        record[0x56..0x58].copy_from_slice(&self.hair.to_le_bytes());
        record[0x68..0x6a].copy_from_slice(&self.hair_color.to_le_bytes());
        record[0x8a] = self.slot;
        record[0x8c] = 1; // Matches the captured character-list record.
        write_field(&mut record[0x8e..0x9e], self.map, "map name")?;
        record[0xae] = self.sex;
        Ok(record)
    }
}

pub struct LoginRequest {
    username: [u8; 24],
    password: [u8; 24],
}

impl LoginRequest {
    pub fn username(&self) -> &[u8] {
        fixed_field(&self.username)
    }

    pub fn password(&self) -> &[u8] {
        fixed_field(&self.password)
    }
}

pub struct CharacterEntry {
    pub account: u32,
    pub login_ids: [u32; 2],
    pub sex: u8,
}

pub enum CharacterRequest {
    Page,
    Keepalive(u32),
    Create(CreateCharacter),
    Select(u8),
}

pub struct CreateCharacter {
    name: [u8; 24],
    pub slot: u8,
    pub hair_color: i16,
    pub hair: i16,
    pub job: i32,
    pub sex: u8,
}

impl CreateCharacter {
    pub fn name(&self) -> &[u8] {
        fixed_field(&self.name)
    }
}

pub fn read_login(reader: &mut impl Read) -> io::Result<LoginRequest> {
    let mut id = read_id(reader)?;
    if id == 0x0204 {
        reader.read_exact(&mut [0; 16])?;
        id = read_id(reader)?;
    }
    if id != 0x0064 {
        return Err(unsupported("login", id));
    }
    let mut body = [0; 53];
    reader.read_exact(&mut body)?;
    Ok(LoginRequest {
        username: body[4..28].try_into().unwrap(),
        password: body[28..52].try_into().unwrap(),
    })
}

pub fn read_character_entry(reader: &mut impl Read) -> io::Result<CharacterEntry> {
    let id = read_id(reader)?;
    if id != 0x0065 {
        return Err(unsupported("character entry", id));
    }
    let mut body = [0; 15];
    reader.read_exact(&mut body)?;
    Ok(CharacterEntry {
        account: u32::from_le_bytes(body[0..4].try_into().unwrap()),
        login_ids: [
            u32::from_le_bytes(body[4..8].try_into().unwrap()),
            u32::from_le_bytes(body[8..12].try_into().unwrap()),
        ],
        sex: body[14],
    })
}

pub fn read_character_request(reader: &mut impl Read) -> io::Result<CharacterRequest> {
    match read_id(reader)? {
        0x09a1 => Ok(CharacterRequest::Page),
        0x0187 => {
            let mut account = [0; 4];
            reader.read_exact(&mut account)?;
            Ok(CharacterRequest::Keepalive(u32::from_le_bytes(account)))
        }
        0x0a39 => {
            let mut body = [0; 34];
            reader.read_exact(&mut body)?;
            Ok(CharacterRequest::Create(CreateCharacter {
                name: body[..24].try_into().unwrap(),
                slot: body[24],
                hair_color: i16::from_le_bytes(body[25..27].try_into().unwrap()),
                hair: i16::from_le_bytes(body[27..29].try_into().unwrap()),
                job: i32::from_le_bytes(body[29..33].try_into().unwrap()),
                sex: body[33],
            }))
        }
        0x0066 => {
            let mut slot = [0];
            reader.read_exact(&mut slot)?;
            Ok(CharacterRequest::Select(slot[0]))
        }
        id => Err(unsupported("character", id)),
    }
}

pub fn login_refused() -> [u8; 26] {
    let mut reply = [0; 26];
    reply[..6].copy_from_slice(&[0x3e, 0x08, 1, 0, 0, 0]);
    reply
}

pub fn character_entry_refused(account: u32) -> [u8; 7] {
    let mut reply = [0; 7];
    reply[..4].copy_from_slice(&account.to_le_bytes());
    reply[4..].copy_from_slice(&[0x6c, 0, 1]);
    reply
}

fn read_id(reader: &mut impl Read) -> io::Result<u16> {
    let mut id = [0; 2];
    reader.read_exact(&mut id)?;
    Ok(u16::from_le_bytes(id))
}

fn fixed_field(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    &bytes[..end]
}

fn write_field(field: &mut [u8], value: &[u8], name: &str) -> io::Result<()> {
    let target = field
        .get_mut(..value.len())
        .ok_or_else(|| invalid_data(format!("{} exceeds wire field width", name)))?;
    target.copy_from_slice(value);
    Ok(())
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn unsupported(stage: &str, id: u16) -> io::Error {
    invalid_data(format!("unsupported {} packet {:#06x}", stage, id))
}

fn variable_header(id: u16, len: usize) -> [u8; 4] {
    // The character server caps a roster at fifteen fixed-size records.
    let [id_low, id_high] = id.to_le_bytes();
    let [len_low, len_high] = (len as u16).to_le_bytes();
    [id_low, id_high, len_low, len_high]
}

pub fn login_reply(
    account: u32,
    login_ids: [u32; 2],
    sex: u8,
    endpoint: SocketAddrV4,
    name: &[u8],
) -> io::Result<[u8; 224]> {
    let mut reply = [0; 224];
    let header = variable_header(0x0ac4, reply.len());
    reply[..4].copy_from_slice(&header);
    reply[4..8].copy_from_slice(&login_ids[0].to_le_bytes());
    reply[8..12].copy_from_slice(&account.to_le_bytes());
    reply[12..16].copy_from_slice(&login_ids[1].to_le_bytes());
    reply[46] = sex;
    reply[64..68].copy_from_slice(&endpoint.ip().octets());
    reply[68..70].copy_from_slice(&endpoint.port().to_le_bytes());
    write_field(&mut reply[70..90], name, "character server name")?;
    Ok(reply)
}

pub fn character_entry(account: u32, slot_count: u8, roster: &[CharacterRecord]) -> Vec<u8> {
    let mut slots = [0; 29];
    slots[..4].copy_from_slice(&[0x2d, 0x08, 29, 0]);
    slots[4] = slot_count;
    slots[7] = slot_count;
    slots[8] = slot_count;
    let mut list = [0; 27];
    let list_header = variable_header(0x006b, list.len() + std::mem::size_of_val(roster));
    list[..4].copy_from_slice(&list_header);
    list[4..7].fill(slot_count);

    let mut reply = account.to_le_bytes().to_vec();
    reply.extend(slots);
    reply.extend(list);
    reply.extend(roster.iter().flatten().copied());
    reply.extend([0xa0, 0x09, 1, 0, 0, 0]); // one roster request, including an empty roster
    reply.extend([0x0d, 0x02, 4, 0]); // empty blocked-character list
    reply.extend([0xb9, 0x08, 0, 0, 0, 0]); // disabled PIN
    reply.extend(account.to_le_bytes());
    reply.extend([0, 0]);
    reply
}

pub fn character_page(roster: &[CharacterRecord]) -> Vec<u8> {
    let mut reply = variable_header(0x0b72, 4 + std::mem::size_of_val(roster)).to_vec();
    reply.extend(roster.iter().flatten().copied());
    if roster.len() == 3 {
        reply.extend(variable_header(0x0b72, 4));
    }
    reply
}

pub fn character_created(record: &CharacterRecord) -> Vec<u8> {
    let mut reply = 0x0b6fu16.to_le_bytes().to_vec();
    reply.extend(record);
    reply
}

pub fn character_creation_refused(reason: u8) -> [u8; 3] {
    [0x6e, 0, reason]
}

pub fn character_selection_refused(owned_slot: bool) -> [u8; 3] {
    [0xca, 0x02, if owned_slot { 4 } else { 0 }]
}

pub struct MapEntry {
    pub account: u32,
    pub character: u32,
    pub auth_code: u32,
    pub sex: u8,
}

pub fn map_destination(
    character: u32,
    map: &[u8],
    endpoint: SocketAddrV4,
) -> io::Result<[u8; 156]> {
    let mut reply = [0; 156];
    reply[..2].copy_from_slice(&0x0ac5u16.to_le_bytes());
    reply[2..6].copy_from_slice(&character.to_le_bytes());
    // Match rAthena's extended map name, including ".gat" and NUL padding.
    write_field(&mut reply[6..22], map, "map name")?;
    reply[22..26].copy_from_slice(&endpoint.ip().octets());
    reply[26..28].copy_from_slice(&endpoint.port().to_le_bytes());
    Ok(reply)
}

pub fn read_map_entry(reader: &mut impl Read) -> io::Result<MapEntry> {
    let id = read_id(reader)?;
    if id != 0x0436 {
        return Err(unsupported("map entry", id));
    }
    let mut body = [0; 21];
    reader.read_exact(&mut body)?;
    Ok(MapEntry {
        account: u32::from_le_bytes(body[..4].try_into().unwrap()),
        character: u32::from_le_bytes(body[4..8].try_into().unwrap()),
        auth_code: u32::from_le_bytes(body[8..12].try_into().unwrap()),
        sex: body[20],
    })
}

pub fn map_account_ack(account: u32) -> [u8; 6] {
    let mut reply = [0; 6];
    reply[..2].copy_from_slice(&0x0283u16.to_le_bytes());
    reply[2..].copy_from_slice(&account.to_le_bytes());
    reply
}

pub fn map_entry_refused() -> [u8; 3] {
    [0x74, 0, 1]
}

pub fn map_accepted(tick: u32, x: u16, y: u16, direction: u8) -> [u8; 13] {
    debug_assert!(x < 1024 && y < 1024 && direction < 8);
    let mut reply = [0; 13];
    reply[..2].copy_from_slice(&0x02ebu16.to_le_bytes());
    reply[2..6].copy_from_slice(&tick.to_le_bytes());
    reply[6] = (x >> 2) as u8;
    reply[7] = ((x & 3) << 6 | y >> 4) as u8;
    reply[8] = ((y & 15) << 4) as u8 | direction;
    reply[9..11].fill(5);
    reply
}

pub enum MapRequest {
    Loaded,
    SyncTime,
    Ping,
    ConfirmLoad,
    RemainingPlaytime,
    CashShopCatalog,
    Quit,
    Face { head: u16, body: u8 },
    Move(u16, u16),
}

pub fn read_map_request(reader: &mut impl Read) -> io::Result<MapRequest> {
    match read_id(reader)? {
        0x007d => Ok(MapRequest::Loaded),
        0x0360 => {
            reader.read_exact(&mut [0; 4])?;
            Ok(MapRequest::SyncTime)
        }
        0x0b1c => Ok(MapRequest::Ping),
        0x0447 => Ok(MapRequest::ConfirmLoad),
        0x01c0 => Ok(MapRequest::RemainingPlaytime),
        0x08c9 => Ok(MapRequest::CashShopCatalog),
        0x018a => {
            reader.read_exact(&mut [0; 2])?;
            Ok(MapRequest::Quit)
        }
        0x0361 => {
            let mut fields = [0; 3];
            reader.read_exact(&mut fields)?;
            Ok(MapRequest::Face {
                head: u16::from_le_bytes([fields[0], fields[1]]),
                body: fields[2],
            })
        }
        0x035f => {
            let mut position = [0; 3];
            reader.read_exact(&mut position)?;
            let [a, b, c] = position.map(u16::from);
            Ok(MapRequest::Move(
                (a << 2) | (b >> 6),
                ((b & 63) << 4) | (c >> 4),
            ))
        }
        id => Err(unsupported("map", id)),
    }
}

pub fn player_move(tick: u32, (x, y): (u16, u16), (dest_x, dest_y): (u16, u16)) -> [u8; 12] {
    let mut reply = [0; 12];
    reply[..2].copy_from_slice(&0x0087u16.to_le_bytes());
    reply[2..6].copy_from_slice(&tick.to_le_bytes());
    reply[6..].copy_from_slice(&packed_walk((x, y), (dest_x, dest_y)));
    reply
}

fn packed_walk((x, y): (u16, u16), (dest_x, dest_y): (u16, u16)) -> [u8; 6] {
    [
        (x >> 2) as u8,
        (((x & 3) << 6) | (y >> 4)) as u8,
        (((y & 15) << 4) | (dest_x >> 6)) as u8,
        (((dest_x & 63) << 2) | (dest_y >> 8)) as u8,
        dest_y as u8,
        0x88,
    ]
}

pub struct PlayerAppearance {
    pub account: u32,
    pub character: u32,
    pub name: String,
    pub job: i16,
    pub hair: i16,
    pub hair_color: i16,
    pub sex: u8,
}

pub fn player_facing(account: u32, head: u16, body: u8) -> [u8; 9] {
    let mut packet = [0x9c, 0, 0, 0, 0, 0, 0, 0, body];
    packet[2..6].copy_from_slice(&account.to_le_bytes());
    packet[6..8].copy_from_slice(&head.to_le_bytes());
    packet
}

pub fn visible_player(
    actor: &PlayerAppearance,
    (x, y): (u16, u16),
    direction: u8,
    head_direction: u16,
    walk: Option<(u32, (u16, u16))>,
) -> Vec<u8> {
    let (opcode, name_offset, palette_offset, sex_offset) = if walk.is_some() {
        (0x09fd, 0x5a, 0x2d, 0x42)
    } else {
        (0x09ff, 0x54, 0x29, 0x3e)
    };
    let mut reply = vec![0; name_offset + 24];
    let header = variable_header(opcode, reply.len());
    reply[..4].copy_from_slice(&header);
    reply[5..9].copy_from_slice(&actor.account.to_le_bytes());
    reply[9..13].copy_from_slice(&actor.character.to_le_bytes());
    reply[13..15].copy_from_slice(&(crate::terrain::WALK_MILLIS as u16).to_le_bytes());
    reply[0x17..0x19].copy_from_slice(&actor.job.to_le_bytes());
    reply[0x19..0x1b].copy_from_slice(&actor.hair.to_le_bytes());
    reply[palette_offset..palette_offset + 2].copy_from_slice(&actor.hair_color.to_le_bytes());
    reply[palette_offset + 4..palette_offset + 6].copy_from_slice(&head_direction.to_le_bytes());
    reply[sex_offset] = actor.sex;
    let level_offset = if let Some((tick, destination)) = walk {
        reply[0x25..0x29].copy_from_slice(&tick.to_le_bytes());
        reply[0x43..0x49].copy_from_slice(&packed_walk((x, y), destination));
        reply[0x49..0x4b].fill(5);
        0x4b
    } else {
        reply[0x3f..0x42].copy_from_slice(&[
            (x >> 2) as u8,
            (((x & 3) << 6) | (y >> 4)) as u8,
            ((y & 15) << 4) as u8 | direction,
        ]);
        reply[0x42..0x44].fill(5);
        0x45
    };
    reply[level_offset] = 1;
    reply[level_offset + 4..level_offset + 12].fill(0xff);
    let name_len = actor.name.len().min(23);
    reply[name_offset..name_offset + name_len].copy_from_slice(&actor.name.as_bytes()[..name_len]);
    reply
}

pub fn player_vanished(account: u32) -> Vec<u8> {
    let mut reply = vec![0x80, 0, 0, 0, 0, 0, 0];
    reply[2..6].copy_from_slice(&account.to_le_bytes());
    reply
}

pub fn map_time(tick: u32) -> [u8; 6] {
    let mut reply = [0; 6];
    reply[..2].copy_from_slice(&0x007fu16.to_le_bytes());
    reply[2..].copy_from_slice(&tick.to_le_bytes());
    reply
}

pub fn map_quit_ack(allowed: bool) -> [u8; 4] {
    [0x8b, 0x01, u8::from(!allowed), 0]
}

pub const MAP_LOAD_CONFIRMED: [u8; 2] = 0x0b1bu16.to_le_bytes();
pub const MAP_PONG: [u8; 2] = 0x0b1du16.to_le_bytes();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_request_and_reply_match_client_layout() {
        let mut input: &[u8] = &[0x61, 0x03, 2, 0, 7, 0x1c, 0x0b];
        assert!(matches!(
            read_map_request(&mut input).unwrap(),
            MapRequest::Face { head: 2, body: 7 }
        ));
        assert!(matches!(
            read_map_request(&mut input).unwrap(),
            MapRequest::Ping
        ));
        assert_eq!(
            player_facing(0x12345678, 2, 7),
            [0x9c, 0, 0x78, 0x56, 0x34, 0x12, 2, 0, 7]
        );
    }

    #[test]
    fn quit_request_consumes_its_full_frame() {
        let mut input: &[u8] = &[0x8a, 0x01, 0, 0, 0x1c, 0x0b];
        assert!(matches!(
            read_map_request(&mut input).unwrap(),
            MapRequest::Quit
        ));
        assert!(matches!(
            read_map_request(&mut input).unwrap(),
            MapRequest::Ping
        ));
        assert_eq!(map_quit_ack(true), [0x8b, 0x01, 0, 0]);
        assert_eq!(map_quit_ack(false), [0x8b, 0x01, 1, 0]);
        let mut truncated: &[u8] = &[0x8a, 0x01, 0];
        assert_eq!(
            read_map_request(&mut truncated).err().unwrap().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn startup_requests_keep_following_move_aligned() {
        let mut requests: &[u8] = &[0xc0, 0x01, 0xc9, 0x08, 0x5f, 0x03, 0x27, 0x4b, 0xf0];
        assert!(matches!(
            read_map_request(&mut requests).unwrap(),
            MapRequest::RemainingPlaytime
        ));
        assert!(matches!(
            read_map_request(&mut requests).unwrap(),
            MapRequest::CashShopCatalog
        ));
        assert!(matches!(
            read_map_request(&mut requests).unwrap(),
            MapRequest::Move(157, 191)
        ));
        assert!(requests.is_empty());
    }

    #[test]
    fn move_request_and_reply_pack_both_tiles() {
        let mut request: &[u8] = &[0x5f, 0x03, 0x27, 0x4b, 0xf0];
        assert!(matches!(
            read_map_request(&mut request).unwrap(),
            MapRequest::Move(157, 191)
        ));
        assert_eq!(
            player_move(0x12345678, (156, 191), (157, 191)),
            [0x87, 0, 0x78, 0x56, 0x34, 0x12, 0x27, 0x0b, 0xf2, 0x74, 0xbf, 0x88]
        );
    }

    #[test]
    fn map_acceptance_encodes_spawn_and_frames_the_load_exchange() {
        assert_eq!(
            map_accepted(0x12345678, 156, 191, 4),
            [0xeb, 0x02, 0x78, 0x56, 0x34, 0x12, 0x27, 0x0b, 0xf4, 5, 5, 0, 0]
        );
        let mut requests: &[u8] = &[0x7d, 0, 0x60, 0x03, 1, 2, 3, 4, 0x1c, 0x0b];
        assert!(matches!(
            read_map_request(&mut requests).unwrap(),
            MapRequest::Loaded
        ));
        assert!(matches!(
            read_map_request(&mut requests).unwrap(),
            MapRequest::SyncTime
        ));
        assert!(matches!(
            read_map_request(&mut requests).unwrap(),
            MapRequest::Ping
        ));
        assert!(requests.is_empty());
    }

    #[test]
    fn map_handoff_uses_rodecomp_packet_layout() {
        let endpoint = "192.0.2.15:5121".parse().unwrap();
        let destination = map_destination(150_001, b"prontera.gat", endpoint).unwrap();
        assert_eq!(&destination[..6], &[0xc5, 0x0a, 0xf1, 0x49, 0x02, 0]);
        assert_eq!(&destination[6..22], b"prontera.gat\0\0\0\0");
        assert_eq!(&destination[22..28], &[192, 0, 2, 15, 1, 20]);
        assert_eq!(&destination[28..], &[0; 128]);

        let mut packet = [0; 23];
        packet[..14].copy_from_slice(&[
            0x36, 0x04, 0x80, 0x84, 0x1e, 0, 0xf1, 0x49, 0x02, 0, 1, 2, 3, 0x80,
        ]);
        packet[14..22].fill(0xff);
        packet[22] = 1;
        let entry = read_map_entry(&mut packet.as_slice()).unwrap();
        assert_eq!(
            (entry.account, entry.character, entry.auth_code, entry.sex),
            (2_000_000, 150_001, 0x8003_0201, 1)
        );
        assert_eq!(
            map_account_ack(entry.account),
            [0x83, 0x02, 0x80, 0x84, 0x1e, 0]
        );
        assert_eq!(
            read_map_entry(&mut &packet[..22]).err().unwrap().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn character_page_matches_capture_and_handles_terminal_pages() {
        let record = CharacterSummary {
            id: 150_000,
            job_level: 1,
            hp: 40,
            max_hp: 40,
            sp: 11,
            max_sp: 11,
            walk_speed: 150,
            base_level: 1,
            name: b"Ferris",
            slot: 0,
            job: 0,
            hair: 0,
            hair_color: 0,
            map: b"prontera.gat",
            sex: 1,
        }
        .encode()
        .unwrap();
        let hex = include_str!("../reference/character-page.hex").trim();
        let expected = (0..hex.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(character_page(&[record]), expected);
        assert_eq!(character_page(&[]), [0x72, 0x0b, 4, 0]);
        let three = character_page(&[record; 3]);
        assert_eq!(&three[..4], &[0x72, 0x0b, 17, 2]);
        assert_eq!(&three[529..], &[0x72, 0x0b, 4, 0]);
    }

    #[test]
    fn create_request_preserves_slot_and_le_fields_before_next_packet() {
        let mut bytes = [0; 38];
        bytes[..2].copy_from_slice(&[0x39, 0x0a]);
        bytes[2..8].copy_from_slice(b"Ferris");
        bytes[26] = 8;
        bytes[27..29].copy_from_slice(&16i16.to_le_bytes());
        bytes[29..31].copy_from_slice(&3i16.to_le_bytes());
        bytes[31..35].copy_from_slice(&4218i32.to_le_bytes());
        bytes[35] = 1;
        bytes[36..].copy_from_slice(&[0xa1, 0x09]);
        let mut input = bytes.as_slice();
        match read_character_request(&mut input).unwrap() {
            CharacterRequest::Create(request) => {
                assert_eq!(request.name(), b"Ferris");
                assert_eq!(
                    (
                        request.slot,
                        request.hair_color,
                        request.hair,
                        request.job,
                        request.sex
                    ),
                    (8, 16, 3, 4218, 1)
                );
            }
            _ => panic!("expected character creation"),
        }
        assert!(matches!(
            read_character_request(&mut input).unwrap(),
            CharacterRequest::Page
        ));
    }
}
