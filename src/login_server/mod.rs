use md5;
use rand::Rng;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

use crate::common::packets::WritablePacket;
fn handle_client(mut stream: TcpStream) {
  let mut data = [0 as u8; 100];
  while match stream.read(&mut data) {
    Ok(size) => {
      if size == 0 {
        return ();
      }

      let command = &data[0..2];
      match command {
        [0xcf, 0xa] => {
          let _packet_size = &data[2..4];
          let _username = &data[6..30];
          let _password_hash = &data[30..];
          let mut login_result = [0 as u8; 3];

          // Server closed packet
          // packet.write_word(0x8100, 0);
          // packet.write_byte(0x01, 2);

          let mut packet = WritablePacket::create(34);

          packet.write_word(0x0ae3, 0); // ACCEPT_LOGIN command
          packet.write_word(34, 2); // Packet size
          packet.write_long(0, 4); // Unknown
          packet.write_str("S1000", 8);
          packet.write_str("token", 28);

          stream.write(&packet.data).unwrap();
        }
        [0xdb, 0x01] => {
          // logclif_parse_reqkey
          let md5digest = md5::compute(b"ragnarok");
          let mut packet = WritablePacket::create(20);
          packet.write_word(0x01dc, 0); // parse req key
          packet.write_word(20, 2);
          packet.write_bytes(&md5digest.0, 4);
          stream.write(&packet.data).unwrap();
        }
        [0xdd, 0x01] => {
          let mut packet = WritablePacket::create(224);
          let random_bytes = rand::thread_rng().gen::<[u8; 17]>();

          packet.write_word(0x0ac4, 0); // Packet ID
          packet.write_word(0x00E0, 2); // Packet size
          packet.write_long(1001, 4); // Login id1
          packet.write_long(1002, 8); // account id
          packet.write_long(1003, 12); // login id2
          packet.write_long(0, 16); // Unknown
          packet.write_byte(1, 46); // account sex
          packet.write_bytes(&random_bytes, 47); // web auth token
          packet.write_long(0x0100007F, 64); // charserver ip
          packet.write_word(0x017E9, 68); // charserver port
          packet.write_str("Rust Athena", 70); // charserver name

          stream.write(&packet.data).unwrap();
        }

        _ => {
          println!("Packet unhandled {:x?}...Shutting down", data);
          stream.shutdown(Shutdown::Both).unwrap();
        }
      }
      // println!(
      //   "Shutting down connection with {}",
      //   stream.peer_addr().unwrap()
      // );
      // stream.shutdown(Shutdown::Both).unwrap();
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
  let listener = TcpListener::bind("0.0.0.0:6900").unwrap();
  println!("Login server is listening on port 6900");

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
