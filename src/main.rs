use std::env;
pub mod common;

mod char_server;
mod login_server;

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() < 2 {
    panic!("Must specify a server argument");
  }

  let server: &str = &args[1];
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
