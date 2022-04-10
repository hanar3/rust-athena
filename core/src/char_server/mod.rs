use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

use crate::common::packets::{ReadablePacket, WritablePacket};

fn handle_client(mut stream: TcpStream) {
  let mut data = [0 as u8; 100];
  while match stream.read(&mut data) {
    Ok(size) => {
      if size == 0 {
        return ();
      }

      let command = &data[0..2];
      match command {
        [0x65, 0x00] => {
          println!("{:x?}", data);

          let read_packet = ReadablePacket::create(&data);
          let account_id: u32 = read_packet.read_long(2);
          let login_id1: u32 = read_packet.read_long(6);
          let login_id2: u32 = read_packet.read_long(10);
          let account_sex: u8 = read_packet.read_byte(16);
          println!(
            "Request connect - account_id {}, login_id1 {}, login_id2 {}, sex {}",
            account_id, login_id1, login_id2, account_sex
          );

          let mut packet = WritablePacket::create(237);
          packet.write_word(0x8480, 0);
          packet.write_word(0x1e, 2);

          packet.write_word(0x82d, 4); // 0x82d
          packet.write_word(29, 6);
          packet.write_byte(15, 8); // normal_slot
          packet.write_byte(0, 9); // premium_slot
          packet.write_byte(0, 10); // billing slot
          packet.write_byte(15, 11); // productible slot
          packet.write_byte(15, 12); // valid_slot

          packet.write_word(0x6b, 33);
          packet.write_word(0xB6, 35); // Size

          packet.write_byte(15, 37); // MAX_CHARS
          packet.write_byte(15, 38); // Available slots
          packet.write_byte(15, 39); // Premium slots
                                     // Char 1
          packet.write_long(150000, 60); // char_id +20 bytes of random offset
          packet.write_longlong(0, 64); // base exp
          packet.write_long(0, 72); // zeny
          packet.write_longlong(0, 76); // job exp
          packet.write_long(1, 84); // job lvl
          packet.write_long(0, 88); // opt1
          packet.write_long(0, 92); // opt2
          packet.write_long(0, 96); // option
          packet.write_long(0, 100); // karma
          packet.write_long(0, 104); // manner
          packet.write_word(48, 108); // status point
          packet.write_long(40, 110); // hp
          packet.write_long(40, 114); // maxhp
          packet.write_word(11, 118); // sp
          packet.write_word(11, 120); // maxsp

          packet.write_word(150, 122); // default walking speed
          packet.write_word(24, 124); // class
          packet.write_word(22, 126); // hair
          packet.write_word(0, 128); // body
          packet.write_word(1, 130); // weapon
          packet.write_word(99, 132); // base lvl
          packet.write_word(0, 134); // skill point
          packet.write_word(0, 136); // head_bottom
          packet.write_word(0, 138); // shield
          packet.write_word(224, 140); // head top
          packet.write_word(0, 142); // head_mid
          packet.write_word(0, 144); // hair color
          packet.write_word(0, 146); // clothes color
          packet.write_str("ragnarok", 148); // name

          packet.write_byte(1, 172); // str -- we add 16 bytes of offset to account for name
          packet.write_byte(1, 173); // agi
          packet.write_byte(1, 174); // vit
          packet.write_byte(1, 175); // int
          packet.write_byte(1, 176); // dex
          packet.write_byte(1, 177); // luk

          packet.write_word(0, 178); // slot
          packet.write_word(1, 180); // rename
          packet.write_str("prontera.gat", 182); // map name
          packet.write_long(0, 198); // Delete date + 16 account for map name size
          packet.write_long(0, 202); // robe
                                     // change slot feature (0 = disabled, otherwise enabled)
          packet.write_long(0, 206);
          packet.write_long(0, 210); //(0 = disabled, otherwise displays "Add-Ons" sidebar)
          packet.write_byte(0, 214); //sex

          // // 0x9a0 - Notify client which page to load
          packet.write_word(0x9a0, 215);
          packet.write_long(5, 217);

          // 0xd2? - Unknown
          packet.write_word(0x20D, 221);
          packet.write_word(4, 223);

          // 0x8b9 - Pincode
          packet.write_word(0x8b9, 225);
          packet.write_long(0xBEE, 227);
          packet.write_long(account_id, 231);
          packet.write_word(0x0, 235);

          stream.write(&packet.data).unwrap();
        }
        [0xa1, 0x09] => {}
        [0x87, 0x1] => {
          // Keepalive packet -- Do nothing for now
        }
        _ => {
          println!("Packet unhandled {:x?}...Shutting down", data);
          stream.shutdown(Shutdown::Both).unwrap();
        }
      }

      true
    }
    Err(_) => {
      println!(
        "An error ocurred, terminating connection with {}",
        stream.peer_addr().unwrap()
      );
      stream.shutdown(Shutdown::Both).unwrap();
      false
    }
  } {}
}

pub fn do_init() {
  let listener = TcpListener::bind("0.0.0.0:6121").unwrap();
  println!("Char server is listening on port 6121");

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        println!("New connection: {}", stream.peer_addr().unwrap());
        thread::spawn(move || handle_client(stream));
      }
      Err(e) => {
        println!("Error: {}", e);
      }
    }
  }

  drop(listener);
}
