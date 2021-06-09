use std::cell::RefCell;

#[derive(Debug)]
pub struct WritablePacket {
  pub data: RefCell<Vec<u8>>,
}

impl WritablePacket {
  pub fn create() -> WritablePacket {
    WritablePacket {
      data: RefCell::new(Vec::new()),
    }
  }

  pub fn write_byte(&self, byte: u8, pos: usize) {
    self.data.borrow_mut().insert(pos, byte);
  }

  pub fn write_word(&mut self, word: u16, mut pos: usize) -> [u8; 2] {
    let bytes = word.to_be_bytes();
    for byte in bytes {
      self.data.borrow_mut().insert(pos, byte);
      pos += 1;
    }
    bytes
  }

  pub fn write_long(&self, long: u32, mut pos: usize) {
    let bytes = long.to_be_bytes();
    println!("{:x?} ", bytes);
    for byte in bytes {
      self.data.borrow_mut().insert(pos, byte);
      pos += 1;
    }
  }

  pub fn write_str(&self, string: &str, mut pos: usize) {
    let bytes = string.as_bytes();

    for byte in bytes {
      self.data.borrow_mut().insert(pos, *byte);
      pos += 1;
    }
  }
}
