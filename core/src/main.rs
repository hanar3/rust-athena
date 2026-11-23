use std::env;
pub mod common;

use rust_athena_db;

mod char_server;
mod demo;
mod login_server;
mod protocol;

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() < 2 {
    panic!("Must specify a server argument");
  }

  let server: &str = &args[1];
  if server == "--demo-20260219" {
    demo::run().expect("could not run the local demo");
    return;
  }
  let db_connection = rust_athena_db::init();

  match server {
    "--server=char" => {
      char_server::do_init();
    }
    "--server=login" => {
      login_server::do_init();
    }
    _ => {}
  }
}
