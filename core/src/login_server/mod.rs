use std::convert::TryFrom;
use std::io::{self, Read, Write};
use std::net::{SocketAddrV4, TcpStream};
use std::time::Duration;

use rust_athena_db::Database;

use crate::{listener, protocol::v20260219 as wire, Result};

pub fn serve(
    db: Database,
    bind: SocketAddrV4,
    destination: SocketAddrV4,
    name: String,
) -> Result<()> {
    listener::serve(bind, Duration::from_secs(30), move |stream| {
        handle(&db, stream, destination, name.as_bytes())
    })
}

fn handle(
    db: &Database,
    mut stream: TcpStream,
    destination: SocketAddrV4,
    name: &[u8],
) -> Result<()> {
    let request = wire::read_login(&mut stream)?;
    let account = match std::str::from_utf8(request.username()) {
        Ok(username) => db.account(username)?,
        Err(_) => None,
    };
    let account = match account {
        Some(account)
            if bcrypt::verify(request.password(), &account.password_hash).unwrap_or(false) =>
        {
            account
        }
        _ => {
            stream.write_all(&wire::login_refused())?;
            return Ok(());
        }
    };
    let account_id = u32::try_from(account.id)?;
    let ids = rand::random();
    let reply = wire::login_reply(account_id, ids, account.sex, destination, name)?;
    db.issue_handoff(account_id, ids, account.sex)?;
    stream.write_all(&reply)?;

    // Rodecomp closes login after choosing a character server. Keep the reply
    // readable until then, with a bounded drain and the listener's read timeout.
    io::copy(&mut stream.take(4096), &mut io::sink())?;
    Ok(())
}
