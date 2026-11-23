use std::io;
use std::net::{SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::Result;

const MAX_CONNECTIONS: usize = 64;

pub fn serve(
    address: SocketAddrV4,
    read_timeout: Duration,
    handle: impl Fn(TcpStream) -> Result<()> + Send + Sync + 'static,
) -> Result<()> {
    let listener = TcpListener::bind(address)?;
    println!("Listening on {}", listener.local_addr()?);
    let active = Arc::new(Mutex::new(0));
    let handle = Arc::new(handle);
    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("Accept failed: {}", error);
                continue;
            }
        };
        let mut count = active.lock().unwrap();
        if *count >= MAX_CONNECTIONS {
            continue;
        }
        *count += 1;
        drop(count);
        let permit = ConnectionPermit(active.clone());
        let handle = handle.clone();
        if let Err(error) = thread::Builder::new().spawn(move || {
            let _permit = permit;
            let result = (|| {
                stream.set_read_timeout(Some(read_timeout))?;
                stream.set_write_timeout(Some(Duration::from_secs(10)))?;
                handle(stream)
            })();
            if let Err(error) = result {
                let normal_disconnect = error.downcast_ref::<io::Error>().is_some_and(|error| {
                    matches!(
                        error.kind(),
                        io::ErrorKind::UnexpectedEof
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::ConnectionReset
                    )
                });
                if !normal_disconnect {
                    eprintln!("Connection failed: {}", error);
                }
            }
        }) {
            eprintln!("Unable to start connection handler: {}", error);
        }
    }
    Ok(())
}

struct ConnectionPermit(Arc<Mutex<usize>>);

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        *self.0.lock().unwrap() -= 1;
    }
}
