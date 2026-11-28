//! Client wire format used by the 2026-02-19 login, character, and map listeners.
use std::io::{self, Cursor, Read};
use std::net::SocketAddrV4;

use deku::{reader::Reader, writer::Writer};
use deku::{DekuRead, DekuReader, DekuWrite, DekuWriter};

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
        Ok(encode_fixed(&RosterCharacter {
            id: self.id,
            job_level: self.job_level,
            hp: self.hp,
            max_hp: self.max_hp,
            sp: self.sp,
            max_sp: self.max_sp,
            walk_speed: self.walk_speed,
            job: self.job,
            hair: self.hair,
            base_level: self.base_level,
            hair_color: self.hair_color,
            name: padded_field(self.name, "character name")?,
            slot: self.slot,
            name_changed: 1,
            map: padded_field(self.map, "map name")?,
            sex: self.sex,
        }))
    }
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct RosterCharacter {
    id: u32,
    #[deku(pad_bytes_before = "20")]
    job_level: i32,
    #[deku(pad_bytes_before = "22")]
    hp: i64,
    max_hp: i64,
    sp: i64,
    max_sp: i64,
    walk_speed: i16,
    job: i16,
    hair: i16,
    #[deku(pad_bytes_before = "4")]
    base_level: i16,
    #[deku(pad_bytes_before = "10", pad_bytes_after = "2")]
    hair_color: i16,
    name: [u8; 24],
    #[deku(pad_bytes_before = "6")]
    slot: u8,
    #[deku(pad_bytes_before = "1")]
    name_changed: i16,
    #[deku(pad_bytes_after = "16")]
    map: [u8; 16],
    sex: u8,
}

#[derive(DekuRead)]

pub struct LoginRequest {
    #[deku(pad_bytes_before = "4")]
    username: [u8; 24],
    #[deku(pad_bytes_after = "1")]
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

#[derive(DekuRead)]
#[deku(endian = "little")]
pub struct CharacterEntry {
    pub account: u32,
    pub login_ids: [u32; 2],
    #[deku(pad_bytes_before = "2")]
    pub sex: u8,
}

pub enum CharacterRequest {
    Page,
    Keepalive(u32),
    Create(CreateCharacter),
    Select(u8),
}

#[derive(DekuRead)]
#[deku(endian = "little")]
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
    decode_body::<_, 53>(reader)
}

pub fn read_character_entry(reader: &mut impl Read) -> io::Result<CharacterEntry> {
    let id = read_id(reader)?;
    if id != 0x0065 {
        return Err(unsupported("character entry", id));
    }
    decode_body::<_, 15>(reader)
}

pub fn read_character_request(reader: &mut impl Read) -> io::Result<CharacterRequest> {
    match read_id(reader)? {
        0x09a1 => Ok(CharacterRequest::Page),
        0x0187 => {
            let mut account = [0; 4];
            reader.read_exact(&mut account)?;
            Ok(CharacterRequest::Keepalive(u32::from_le_bytes(account)))
        }
        0x0a39 => decode_body::<_, 34>(reader).map(CharacterRequest::Create),
        0x0066 => {
            let mut slot = [0];
            reader.read_exact(&mut slot)?;
            Ok(CharacterRequest::Select(slot[0]))
        }
        id => Err(unsupported("character", id)),
    }
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct LoginRefused {
    opcode: u16,
    #[deku(pad_bytes_after = "20")]
    result: u32,
}

pub fn login_refused() -> [u8; 26] {
    encode_fixed(&LoginRefused {
        opcode: 0x083e,
        result: 1,
    })
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct CharacterEntryRefused {
    account: u32,
    opcode: u16,
    reason: u8,
}

pub fn character_entry_refused(account: u32) -> [u8; 7] {
    encode_fixed(&CharacterEntryRefused {
        account,
        opcode: 0x006c,
        reason: 1,
    })
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

fn padded_field<const N: usize>(value: &[u8], name: &str) -> io::Result<[u8; N]> {
    let mut field = [0; N];
    let target = field
        .get_mut(..value.len())
        .ok_or_else(|| invalid_data(format!("{} exceeds wire field width", name)))?;
    target.copy_from_slice(value);
    Ok(field)
}

fn decode_body<T, const N: usize>(reader: &mut impl Read) -> io::Result<T>
where
    T: for<'a> DekuReader<'a, ()>,
{
    // Keep the socket read bounded to the current frame. Deku can seek within the body.
    let mut bytes = [0; N];
    reader.read_exact(&mut bytes)?;
    T::from_reader_with_ctx(&mut Reader::new(Cursor::new(bytes)), ())
        .map_err(|error| invalid_data(error.to_string()))
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn unsupported(stage: &str, id: u16) -> io::Error {
    invalid_data(format!("unsupported {} packet {:#06x}", stage, id))
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct VariableHeader {
    opcode: u16,
    length: u16,
}

impl VariableHeader {
    fn new(opcode: u16, length: usize) -> Self {
        Self {
            opcode,
            length: length as u16,
        }
    }
}

#[derive(DekuWrite)]
struct LoginAccepted {
    header: VariableHeader,
    #[deku(endian = "little")]
    login_id1: u32,
    #[deku(endian = "little")]
    account: u32,
    #[deku(endian = "little", pad_bytes_after = "30")]
    login_id2: u32,
    #[deku(pad_bytes_after = "17")]
    sex: u8,
    address: [u8; 4],
    #[deku(endian = "little")]
    port: u16,
    #[deku(pad_bytes_after = "134")]
    name: [u8; 20],
}

pub fn login_reply(
    account: u32,
    login_ids: [u32; 2],
    sex: u8,
    endpoint: SocketAddrV4,
    name: &[u8],
) -> io::Result<[u8; 224]> {
    Ok(encode_fixed(&LoginAccepted {
        header: VariableHeader::new(0x0ac4, 224),
        login_id1: login_ids[0],
        account,
        login_id2: login_ids[1],
        sex,
        address: endpoint.ip().octets(),
        port: endpoint.port(),
        name: padded_field(name, "character server name")?,
    }))
}

#[derive(DekuWrite)]
struct CharacterSlots {
    header: VariableHeader,
    normal: u8,
    #[deku(pad_bytes_before = "2")]
    valid: u8,
    #[deku(pad_bytes_after = "20")]
    total: u8,
}

#[derive(DekuWrite)]
struct CharacterListHeader {
    header: VariableHeader,
    max_slots: u8,
    available_slots: u8,
    #[deku(pad_bytes_after = "20")]
    premium_slots: u8,
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct RosterPageCount {
    opcode: u16,
    pages: u32,
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct PinState {
    opcode: u16,
    seed: u32,
    account: u32,
    state: u16,
}

pub fn character_entry(account: u32, slot_count: u8, roster: &[CharacterRecord]) -> Vec<u8> {
    let slots: [u8; 29] = encode_fixed(&CharacterSlots {
        header: VariableHeader::new(0x082d, 29),
        normal: slot_count,
        valid: slot_count,
        total: slot_count,
    });
    let list: [u8; 27] = encode_fixed(&CharacterListHeader {
        header: VariableHeader::new(0x006b, 27 + std::mem::size_of_val(roster)),
        max_slots: slot_count,
        available_slots: slot_count,
        premium_slots: slot_count,
    });
    let count: [u8; 6] = encode_fixed(&RosterPageCount {
        opcode: 0x09a0,
        pages: 1,
    });
    let blocked: [u8; 4] = encode_fixed(&VariableHeader::new(0x020d, 4));
    let pin: [u8; 12] = encode_fixed(&PinState {
        opcode: 0x08b9,
        seed: 0,
        account,
        state: 0,
    });

    let mut reply = account.to_le_bytes().to_vec();
    reply.extend(slots);
    reply.extend(list);
    reply.extend(roster.iter().flatten().copied());
    reply.extend(count);
    reply.extend(blocked);
    reply.extend(pin);
    reply
}

pub fn character_page(roster: &[CharacterRecord]) -> Vec<u8> {
    let header: [u8; 4] = encode_fixed(&VariableHeader::new(
        0x0b72,
        4 + std::mem::size_of_val(roster),
    ));
    let mut reply = header.to_vec();
    reply.extend(roster.iter().flatten().copied());
    if roster.len() == 3 {
        let terminal: [u8; 4] = encode_fixed(&VariableHeader::new(0x0b72, 4));
        reply.extend(terminal);
    }
    reply
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct CharacterCreated {
    opcode: u16,
    record: CharacterRecord,
}

pub fn character_created(record: &CharacterRecord) -> Vec<u8> {
    let bytes: [u8; 177] = encode_fixed(&CharacterCreated {
        opcode: 0x0b6f,
        record: *record,
    });
    bytes.to_vec()
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct ByteResult {
    opcode: u16,
    result: u8,
}

pub fn character_creation_refused(reason: u8) -> [u8; 3] {
    encode_fixed(&ByteResult {
        opcode: 0x006e,
        result: reason,
    })
}

pub fn character_selection_refused(owned_slot: bool) -> [u8; 3] {
    encode_fixed(&ByteResult {
        opcode: 0x02ca,
        result: if owned_slot { 4 } else { 0 },
    })
}

#[derive(DekuRead)]
#[deku(endian = "little")]

pub struct MapEntry {
    pub account: u32,
    pub character: u32,
    pub auth_code: u32,
    #[deku(pad_bytes_before = "8")]
    pub sex: u8,
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct MapDestination {
    opcode: u16,
    character: u32,
    map: [u8; 16],
    address: [u8; 4],
    #[deku(pad_bytes_after = "128")]
    port: u16,
}

pub fn map_destination(
    character: u32,
    map: &[u8],
    endpoint: SocketAddrV4,
) -> io::Result<[u8; 156]> {
    Ok(encode_fixed(&MapDestination {
        opcode: 0x0ac5,
        character,
        map: padded_field(map, "map name")?,
        address: endpoint.ip().octets(),
        port: endpoint.port(),
    }))
}

pub fn read_map_entry(reader: &mut impl Read) -> io::Result<MapEntry> {
    let id = read_id(reader)?;
    if id != 0x0436 {
        return Err(unsupported("map entry", id));
    }
    decode_body::<_, 21>(reader)
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct MapAccountAck {
    opcode: u16,
    account: u32,
}

pub fn map_account_ack(account: u32) -> [u8; 6] {
    encode_fixed(&MapAccountAck {
        opcode: 0x0283,
        account,
    })
}

pub fn map_entry_refused() -> [u8; 3] {
    encode_fixed(&ByteResult {
        opcode: 0x0074,
        result: 1,
    })
}

#[derive(DekuRead, DekuWrite)]
#[deku(endian = "big", bit_order = "msb")]
struct MapPosition {
    #[deku(bits = "10")]
    x: u16,
    #[deku(bits = "10")]
    y: u16,
    /// Facing direction in server replies. Unused by the walk request handler.
    #[deku(bits = "4")]
    direction: u8,
}

#[derive(DekuWrite)]
struct MapAccepted {
    #[deku(endian = "little")]
    opcode: u16,
    #[deku(endian = "little")]
    tick: u32,
    position: MapPosition,
    sight_width: u8,
    #[deku(pad_bytes_after = "2")]
    sight_height: u8,
}

pub fn map_accepted(tick: u32, x: u16, y: u16, direction: u8) -> [u8; 13] {
    debug_assert!(x < 1024 && y < 1024 && direction < 8);
    encode_fixed(&MapAccepted {
        opcode: 0x02eb,
        tick,
        position: MapPosition { x, y, direction },
        sight_width: 5,
        sight_height: 5,
    })
}

#[derive(DekuRead)]
#[deku(endian = "little")]
struct ChangeDirection {
    head: u16,
    body: u8,
}

pub enum MapRequest {
    Loaded,
    SyncTime,
    Ping,
    ConfirmLoad,
    RemainingPlaytime,
    CashShopCatalog,
    Quit,
    Restart(u8),
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
        0x00b2 => {
            let mut kind = [0];
            reader.read_exact(&mut kind)?;
            Ok(MapRequest::Restart(kind[0]))
        }
        0x018a => {
            reader.read_exact(&mut [0; 2])?;
            Ok(MapRequest::Quit)
        }
        0x0361 => {
            let ChangeDirection { head, body } = decode_body::<_, 3>(reader)?;
            Ok(MapRequest::Face { head, body })
        }
        0x035f => {
            let position: MapPosition = decode_body::<_, 3>(reader)?;
            Ok(MapRequest::Move(position.x, position.y))
        }

        id => Err(unsupported("map", id)),
    }
}

pub fn player_move(tick: u32, source: (u16, u16), destination: (u16, u16)) -> [u8; 12] {
    let packet = NotifyPlayerMove {
        opcode: NotifyPlayerMove::OPCODE,
        walk_start_tick: tick,
        route: WalkRoute::new(source, destination),
    };
    encode_fixed(&packet)
}

#[derive(DekuWrite)]
struct NotifyPlayerMove {
    #[deku(endian = "little")]
    opcode: u16,
    #[deku(endian = "little")]
    walk_start_tick: u32,
    route: WalkRoute,
}

impl NotifyPlayerMove {
    const OPCODE: u16 = 0x0087;
}

#[derive(DekuWrite)]
#[deku(endian = "big", bit_order = "msb")]
struct WalkRoute {
    #[deku(bits = "10")]
    source_x: u16,
    #[deku(bits = "10")]
    source_y: u16,
    #[deku(bits = "10")]
    destination_x: u16,
    #[deku(bits = "10")]
    destination_y: u16,
    #[deku(bits = "4")]
    sub_tile_x: u8,
    #[deku(bits = "4")]
    sub_tile_y: u8,
}

impl WalkRoute {
    fn new((source_x, source_y): (u16, u16), (destination_x, destination_y): (u16, u16)) -> Self {
        const SUB_TILE_CENTER: u8 = 8;

        Self {
            source_x,
            source_y,
            destination_x,
            destination_y,
            sub_tile_x: SUB_TILE_CENTER,
            sub_tile_y: SUB_TILE_CENTER,
        }
    }
}

fn encode_fixed<const N: usize>(packet: &impl DekuWriter<()>) -> [u8; N] {
    let mut bytes = [0; N];
    let mut cursor = Cursor::new(bytes.as_mut_slice());
    // Packet fields are bounded before encoding, including coordinates and strings.
    packet
        .to_writer(&mut Writer::new(&mut cursor), ())
        .expect("valid fixed-size packet");
    debug_assert_eq!(cursor.position(), N as u64);
    bytes
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

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct PlayerFacing {
    opcode: u16,
    account: u32,
    head: u16,
    body: u8,
}

pub fn player_facing(account: u32, head: u16, body: u8) -> [u8; 9] {
    encode_fixed(&PlayerFacing {
        opcode: 0x009c,
        account,
        head,
        body,
    })
}

#[derive(DekuWrite)]
struct ActorEntryHeader {
    header: VariableHeader,
    #[deku(endian = "little", pad_bytes_before = "1")]
    account: u32,
    #[deku(endian = "little")]
    character: u32,
    #[deku(endian = "little", pad_bytes_after = "8")]
    walk_speed: u16,
    #[deku(endian = "little")]
    job: i16,
    #[deku(endian = "little")]
    hair: i16,
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct ActorEntryLook {
    #[deku(pad_bytes_after = "2")]
    hair_color: i16,
    #[deku(pad_bytes_after = "15")]
    head_direction: u16,
    sex: u8,
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct ActorEntryTail {
    #[deku(pad_bytes_after = "2")]
    level: i16,
    max_hp: i32,
    #[deku(pad_bytes_after = "3")]
    hp: i32,
    name: [u8; 24],
}

#[derive(DekuWrite)]
struct StationaryActor {
    header: ActorEntryHeader,
    #[deku(pad_bytes_before = "14")]
    look: ActorEntryLook,
    position: MapPosition,
    width: u8,
    height: u8,
    state: u8,
    tail: ActorEntryTail,
}

#[derive(DekuWrite)]
struct MovingActor {
    header: ActorEntryHeader,
    #[deku(endian = "little", pad_bytes_before = "10", pad_bytes_after = "4")]
    walk_start_tick: u32,
    look: ActorEntryLook,
    route: WalkRoute,
    width: u8,
    height: u8,
    tail: ActorEntryTail,
}

pub fn visible_player(
    actor: &PlayerAppearance,
    (x, y): (u16, u16),
    direction: u8,
    head_direction: u16,
    walk: Option<(u32, (u16, u16))>,
) -> Vec<u8> {
    let header = |opcode, length| ActorEntryHeader {
        header: VariableHeader::new(opcode, length),
        account: actor.account,
        character: actor.character,
        walk_speed: crate::terrain::WALK_MILLIS as u16,
        job: actor.job,
        hair: actor.hair,
    };
    let look = ActorEntryLook {
        hair_color: actor.hair_color,
        head_direction,
        sex: actor.sex,
    };
    let mut name = [0; 24];
    let name_len = actor.name.len().min(23);
    name[..name_len].copy_from_slice(&actor.name.as_bytes()[..name_len]);

    let tail = ActorEntryTail {
        level: 1,
        max_hp: -1,
        hp: -1,
        name,
    };

    if let Some((tick, destination)) = walk {
        let bytes: [u8; 114] = encode_fixed(&MovingActor {
            header: header(0x09fd, 114),
            walk_start_tick: tick,
            look,
            route: WalkRoute::new((x, y), destination),
            width: 5,
            height: 5,
            tail,
        });
        bytes.to_vec()
    } else {
        let bytes: [u8; 108] = encode_fixed(&StationaryActor {
            header: header(0x09ff, 108),
            look,
            position: MapPosition { x, y, direction },
            width: 5,
            height: 5,
            state: 0,
            tail,
        });
        bytes.to_vec()
    }
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct PlayerVanished {
    opcode: u16,
    account: u32,
    reason: u8,
}

pub fn player_vanished(account: u32) -> Vec<u8> {
    let bytes: [u8; 7] = encode_fixed(&PlayerVanished {
        opcode: 0x0080,
        account,
        reason: 0,
    });
    bytes.to_vec()
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct MapTime {
    opcode: u16,
    tick: u32,
}

pub fn map_time(tick: u32) -> [u8; 6] {
    encode_fixed(&MapTime {
        opcode: 0x007f,
        tick,
    })
}

#[derive(DekuWrite)]
#[deku(endian = "little")]
struct MapQuitAck {
    opcode: u16,
    result: u16,
}

pub const MAP_RETURN_TO_CHARACTER_SELECT: [u8; 3] = [0xb3, 0, 1];

pub fn map_quit_ack(allowed: bool) -> [u8; 4] {
    encode_fixed(&MapQuitAck {
        opcode: 0x018b,
        result: u16::from(!allowed),
    })
}

pub const MAP_LOAD_CONFIRMED: [u8; 2] = 0x0b1bu16.to_le_bytes();
pub const MAP_PONG: [u8; 2] = 0x0b1du16.to_le_bytes();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_preamble_preserves_frame_alignment_and_truncation() {
        let mut frames = vec![0x04, 0x02];
        frames.extend([0xaa; 16]);
        frames.extend([0x64, 0, 1, 2, 3, 4]);
        frames.extend(padded_field::<24>(b"TestAccount", "username").unwrap());
        frames.extend(padded_field::<24>(b"test-password", "password").unwrap());
        frames.push(7);
        let login_len = frames.len();
        frames.extend([0x1c, 0x0b]);

        let mut reader = frames.as_slice();
        let login = read_login(&mut reader).unwrap();
        assert_eq!(login.username(), b"TestAccount");
        assert_eq!(login.password(), b"test-password");
        assert!(matches!(
            read_map_request(&mut reader).unwrap(),
            MapRequest::Ping
        ));

        for len in 0..login_len {
            let error = read_login(&mut &frames[..len]).err().unwrap();
            assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        }
    }

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
    fn restart_request_consumes_one_type_byte() {
        for kind in [0, 1, 255] {
            let frame = [0xb2, 0, kind, 0x1c, 0x0b];
            let mut reader = frame.as_slice();
            assert!(
                matches!(read_map_request(&mut reader).unwrap(), MapRequest::Restart(value) if value == kind)
            );
            assert!(matches!(
                read_map_request(&mut reader).unwrap(),
                MapRequest::Ping
            ));
        }
        assert!(read_map_request(&mut &[0xb2, 0][..]).is_err());
        assert_eq!(MAP_RETURN_TO_CHARACTER_SELECT, [0xb3, 0, 1]);
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
    fn self_move_and_moving_actor_encode_high_coordinate_bits() {
        let tick = 0x12345678;
        let source = (1023, 512);
        let destination = (768, 1023);
        assert_eq!(
            player_move(tick, source, destination),
            [0x87, 0, 0x78, 0x56, 0x34, 0x12, 0xff, 0xe0, 0x0c, 0x03, 0xff, 0x88]
        );

        let actor = PlayerAppearance {
            account: 1,
            character: 2,
            name: "Walker".into(),
            job: 0,
            hair: 0,
            hair_color: 0,
            sex: 0,
        };
        let packet = visible_player(&actor, source, 0, 0, Some((tick, destination)));
        assert_eq!(&packet[..4], &[0xfd, 0x09, 0x72, 0]);
        assert_eq!(&packet[0x25..0x29], &[0x78, 0x56, 0x34, 0x12]);
        assert_eq!(&packet[0x43..0x49], &[0xff, 0xe0, 0x0c, 0x03, 0xff, 0x88]);
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
