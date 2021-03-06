extern crate diesel;
extern crate serde_derive;
extern crate serde_json;
use md5;
use rand::Rng;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use rust_athena_db::models::User;
use rust_athena_db::schema::users;
use rust_athena_db::schema::users::dsl::*;

use std::thread;
use diesel::prelude::*;
use diesel::{Insertable};
use serde::{Deserialize, Serialize};

use crate::common::packets::WritablePacket;

#[derive(Serialize, Deserialize, Insertable)]
#[table_name = "users"]
pub struct UserForm{
  pub username:  String,
  pub password_hash:  String,
  pub email:  String,
}

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
          // let mut packet = WritablePacket::create(224);
          // let web_auth_token = rand::thread_rng().gen::<[u8; 17]>();

          // packet.write_word(0x0ac4, 0); // Packet ID
          // packet.write_word(0x00E0, 2); // Packet size
          // packet.write_long(2000000, 4); // Login id1
          // packet.write_long(2000000, 8); // account id
          // packet.write_long(2000000, 12); // login id2
          // packet.write_long(0, 16); // Unknown
          // packet.write_byte(1, 46); // account sex
          // packet.write_bytes(&web_auth_token, 47); // web auth token
          // packet.write_long(0x0100007F, 64); // charserver ip
          // packet.write_word(0x017E9, 68); // charserver port
          // packet.write_str("Rust Athena", 70); // charserver name

          // stream.write(&packet.data).unwrap();
        }
        [0x64, 0x0] => {
          let mut packet = WritablePacket::create(224);
          let web_auth_token = rand::thread_rng().gen::<[u8; 17]>();

          packet.write_word(0x0ac4, 0); // Packet ID
          packet.write_word(0x00E0, 2); // Packet size
          packet.write_long(2000000, 4); // Login id1
          packet.write_long(2000000, 8); // account id
          packet.write_long(2000000, 12); // login id2
          packet.write_long(0, 16); // Unknown
          packet.write_byte(1, 46); // account sex
          packet.write_bytes(&web_auth_token, 47); // web auth token
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
  let conn = rust_athena_db::connection().unwrap();


  let user_form = UserForm{
    username: "s1".to_string(),
    password_hash: "p1".to_string(),
    email: "a@a.com".to_string(),
  };

  let query = diesel::insert_or_ignore_into(users::table).values(&user_form).execute(&conn).unwrap();
  println!("Login server is listening on port 6900 -- MYSQL Listening on port 3306");

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
