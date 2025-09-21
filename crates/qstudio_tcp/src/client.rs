use busbar::Copper;
use std::io::Write;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
#[derive(Debug, Clone)]
pub struct Client {
    pub addr: String,
}

impl Client {
    pub fn new(addr: String) -> Self {
        log::info!("Creating client for {}", addr);
        Client { addr }
    }

    pub fn send<T>(&self, message: Copper<T>) -> std::io::Result<()>
    where
        T: serde::Serialize,
    {
        log::info!("Client sending to {}", self.addr);
        if let Some(mut stream) = retry_connect(&self.addr, 5, Duration::from_secs(1)) {
            let json_message = match message.to_json() {
                Ok(json) => json,
                Err(e) => {
                    log::error!("Error serializing message: {}", e);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Serialization error",
                    ));
                }
            };
            stream.write_all(json_message.as_bytes())?;
            stream.write_all(b"\n")?;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to connect after retries",
            ))
        }
    }
}

fn retry_connect(addr: &str, attempts: usize, backoff: Duration) -> Option<TcpStream> {
    for n in 1..=attempts {
        match TcpStream::connect(addr) {
            Ok(s) => return Some(s),
            Err(e) => {
                if n == attempts {
                    eprintln!("[SDK] Final connect error: {e}");
                    return None;
                }
                thread::sleep(backoff);
            }
        }
    }
    None
}
