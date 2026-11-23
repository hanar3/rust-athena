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
    pub rename_count: i16,
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
        record[0x8c..0x8e].copy_from_slice(&self.rename_count.to_le_bytes());
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
    Keepalive,
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
            reader.read_exact(&mut [0; 4])?;
            Ok(CharacterRequest::Keepalive)
        }
        id => Err(unsupported("character", id)),
    }
}

pub fn login_refused() -> [u8; 23] {
    // Keep the demo's refusal format, which Rodecomp recognizes.
    let mut reply = [0; 23];
    reply[..3].copy_from_slice(&[0x6a, 0, 1]);
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
    // Callers serialize fixed-size layouts smaller than u16::MAX.
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

pub fn character_entry(account: u32, slot_count: u8, character: &CharacterRecord) -> Vec<u8> {
    let mut slots = [0; 29];
    slots[..4].copy_from_slice(&[0x2d, 0x08, 29, 0]);
    slots[4] = slot_count;
    slots[7] = slot_count;
    slots[8] = slot_count;
    let mut list = [0; 27];
    let list_header = variable_header(0x006b, list.len() + character.len());
    list[..4].copy_from_slice(&list_header);
    list[4..7].fill(slot_count);

    let mut reply = account.to_le_bytes().to_vec();
    reply.extend(slots);
    reply.extend(list);
    reply.extend(character);
    reply.extend([0xa0, 0x09, 1, 0, 0, 0]); // one character page available
    reply.extend([0x0d, 0x02, 4, 0]); // empty blocked-character list
    reply.extend([0xb9, 0x08, 0, 0, 0, 0]); // disabled PIN
    reply.extend(account.to_le_bytes());
    reply.extend([0, 0]);
    reply
}

pub fn character_page(character: &CharacterRecord) -> Vec<u8> {
    let mut reply = variable_header(0x0b72, 4 + character.len()).to_vec();
    reply.extend(character);
    reply
}
