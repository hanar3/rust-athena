mod char_server;
mod listener;
mod login_server;
mod protocol;

use std::env;
use std::io::{self, BufRead};
use std::net::SocketAddrV4;

use rust_athena_db::Database;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    dotenv::dotenv().ok();
    let args: Vec<_> = env::args().skip(1).collect();
    let version = env::var("ATHENA_PACKETVER").unwrap_or_else(|_| "20260219".into());
    if version != "20260219" {
        return Err(format!("Unsupported ATHENA_PACKETVER: {}", version).into());
    }

    match args.as_slice() {
        [command] if command == "login" => {
            let bind = address("ATHENA_LOGIN_BIND", "127.0.0.1:6900")?;
            let destination = address("ATHENA_CHAR_ADVERTISE", "127.0.0.1:6121")?;
            if destination.ip().is_unspecified() || destination.port() == 0 {
                return Err(
                    "ATHENA_CHAR_ADVERTISE must be a concrete client-reachable endpoint".into(),
                );
            }
            let name = env::var("ATHENA_CHAR_NAME").unwrap_or_else(|_| "Rust Athena".into());
            if name.is_empty() || name.len() > 20 || !name.is_ascii() {
                return Err("ATHENA_CHAR_NAME must be 1–20 ASCII bytes".into());
            }
            login_server::serve(database()?, bind, destination, name)
        }
        [command] if command == "char" => {
            char_server::serve(database()?, address("ATHENA_CHAR_BIND", "127.0.0.1:6121")?)
        }
        [command, username, email] if command == "register" => register(username, email),
        _ => Err(
            "Usage: rust-athena login | char | register <username> <email> (password on stdin)"
                .into(),
        ),
    }
}

fn database() -> Result<Database> {
    Database::connect(&env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required")?)
}

fn address(key: &str, default: &str) -> Result<SocketAddrV4> {
    Ok(env::var(key)
        .unwrap_or_else(|_| default.into())
        .parse()
        .map_err(|error| format!("{}: {}", key, error))?)
}

fn register(username: &str, email: &str) -> Result<()> {
    if !(4..=23).contains(&username.len())
        || !username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err("Username must be 4–23 ASCII letters, digits, or underscores".into());
    }
    if email.is_empty() || email.len() > 255 {
        return Err("Email must contain 1–255 bytes".into());
    }
    let mut password = String::new();
    io::stdin().lock().read_line(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if !(8..=23).contains(&password.len()) || !password.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err("Password must be 8–23 printable ASCII characters without spaces".into());
    }
    database()?.register(
        username,
        &bcrypt::hash(password, bcrypt::DEFAULT_COST)?,
        email,
    )?;
    println!("Account registered");
    Ok(())
}
