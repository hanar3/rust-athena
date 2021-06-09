use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

use crate::common::WritablePacket;
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

          // Server closed result
          login_result[0] = 0x81; // command
          login_result[1] = 0x00; // command
          login_result[2] = 0x01;

          // AC_ACCEPT_LOGIN3
          let accept_login = [
            0xe3, 0x0a, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x53, 0x31, 0x30, 0x30, 0x30, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x74, 0x6f, 0x6b, 0x65, 0x6e, 0x00,
          ];

          stream.write(&accept_login).unwrap();
        }
        [0xdb, 0x01] => {
          // logclif_parse_reqkey
          let cl_key = [
            0xdc, 0x01, 0x13, 0x00, 0x96, 0x02, 0x4d, 0x03, 0x10, 0xb3, 0x5a, 0x89, 0x10, 0x25,
            0x4c, 0xe0, 0x44, 0x89, 0x57,
          ];
          stream.write(&cl_key).unwrap();
        }
        [0xdd, 0x01] => {
          // logclif_parse_auth
          let clif_auth = [
            0xC4, 0x0A, 0xE0, 0x00, 0xE0, 0x71, 0xF4, 0x44, 0x80, 0x84, 0x1E, 0x00, 0x8C, 0xB9,
            0x8A, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x30, 0x31, 0x62, 0x31, 0x34, 0x30, 0x39, 0x39, 0x66,
            0x66, 0x33, 0x38, 0x62, 0x65, 0x39, 0x65, 0x00, 0x7F, 0x00, 0x00, 0x01, 0xE9, 0x17,
            0x52, 0x75, 0x73, 0x74, 0x53, 0x65, 0x72, 0x76, 0x65, 0x72, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
          ];
          stream.write(&clif_auth).unwrap();
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
