extern crate byteorder;
use byteorder::{LittleEndian, WriteBytesExt};
use std::boxed::Box;

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
}
