extern crate byteorder;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::boxed::Box;
use std::io::Cursor;

#[derive(Debug)]
pub struct WritablePacket {
  pub data: Box<[u8]>,
}
impl WritablePacket {
  pub fn create(size: usize) -> WritablePacket {
    return WritablePacket {
      data: vec![0 as u8; size].into_boxed_slice(),
    };
  }

  pub fn write_byte(&mut self, byte: u8, pos: usize) {
    self.data[pos] = byte;
  }

  pub fn write_word(&mut self, word: u16, mut pos: usize) {
    let mut bytes = vec![];
    bytes.write_u16::<LittleEndian>(word).unwrap();

    for byte in bytes {
      self.data[pos] = byte;
      pos += 1;
    }
  }

  pub fn write_long(&mut self, long: u32, mut pos: usize) {
    let mut bytes = vec![];
    bytes.write_u32::<LittleEndian>(long).unwrap();
    for byte in bytes {
      self.data[pos] = byte;
      pos += 1;
    }
  }

  pub fn write_str(&mut self, s: &str, mut pos: usize) {
    let mut bytes = vec![0u8; s.len()];
    bytes[0..s.len()].copy_from_slice(s.as_bytes());

    for byte in bytes {
      self.data[pos] = byte;
      pos += 1;
    }
  }

  pub fn write_bytes(&mut self, b: &[u8], mut pos: usize) {
    let mut bytes = vec![0u8; b.len()];
    bytes[0..b.len()].copy_from_slice(&b[..]);
    for byte in bytes {
      self.data[pos] = byte;
      pos += 1;
    }
  }
}

pub struct ReadablePacket {
  pub data: Box<[u8]>,
}

impl ReadablePacket {
  pub fn create(packet: &[u8]) -> ReadablePacket {
    let mut bytes = vec![0u8; packet.len()];
    bytes[0..packet.len()].copy_from_slice(&packet[..]);

    return ReadablePacket {
      data: bytes.into_boxed_slice(),
    };
  }

  pub fn read_long(&self, mut pos: usize) -> u32 {
    let mut bytes = [0u8; 4];
    for n in 0..bytes.len() {
      bytes[n] = self.data[pos];
      pos += 1;
    }

    let mut rdr = Cursor::new(bytes);
    let long = rdr.read_u32::<LittleEndian>().unwrap();

    return long;
  }

  pub fn read_word(&self, mut pos: usize) -> u16 {
    let mut bytes = [0u8; 2];
    for n in 0..bytes.len() {
      bytes[n] = self.data[pos];
      pos += 1;
    }

    let mut rdr = Cursor::new(bytes);
    let word = rdr.read_u16::<LittleEndian>().unwrap();

    return word;
  }

  pub fn read_byte(&self, mut pos: usize) -> u8 {
    return self.data[pos];
  }
}
