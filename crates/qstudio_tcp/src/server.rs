use busbar::{Copper, MakeT, Response, Unravel};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};

pub struct Server {
    pub rx_addr: String,
    pub tx_addr: String,
}

impl Server {
    pub fn new(rx_addr: String, tx_addr: String) -> Self {
        Server { rx_addr, tx_addr }
    }

    pub fn listen<A, T: Unravel<A, T, B>, B: Response<A, B>>(&self, tx: HashMap<A, Sender<T>>)
    where
        T: for<'de> serde::Deserialize<'de> + std::fmt::Display,
        A: std::hash::Hash + Eq + Clone + std::fmt::Debug,
        B: MakeT<T>,
    {
        let listener = TcpListener::bind(&self.rx_addr).unwrap();
        log::info!("Server listening on {}", self.rx_addr);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    for b in self.incoming::<A, T, B>(stream, &tx) {
                        log::info!("Response: {}", b.message());

                        log::info!("Found sender for event type: {:?}", b.event_type());
                        if let Some(sender) = tx.get(&b.event_type()) {
                            let event = b.make_t();

                            if let Err(e) = sender.send(event) {
                                log::error!("Error sending to backend: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    fn incoming<A, T, B>(&self, stream: TcpStream, tx: &HashMap<A, Sender<T>>) -> Vec<B>
    where
        A: std::hash::Hash + Eq + Clone + std::fmt::Debug,
        T: for<'de> serde::Deserialize<'de> + std::fmt::Display + Unravel<A, T, B>,
        B: Response<A, B>,
    {
        let addr = stream.peer_addr().unwrap();
        log::info!("Client connected: {}", addr);

        let mut res = Vec::new();

        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(message) => {
                    let copper: Copper<T> = Copper::from_json(&message).unwrap();
                    res.push(self.handle_message(copper, tx));
                }
                Err(e) => {
                    log::error!("Error reading from client {}: {}", addr, e);
                    return vec![B::default()];
                }
            }
        }
        res
    }

    fn handle_message<A, T: Unravel<A, T, B>, B: Response<A, B>>(
        &self,
        message: Copper<T>,
        tx: &HashMap<A, Sender<T>>,
    ) -> B
    where
        A: Eq + std::hash::Hash + Clone + std::fmt::Debug,
        B: Response<A, B>,
    {
        match message {
            Copper::ToClient { .. } => return B::default(),
            Copper::ToServer { payload, .. } => {
                // do something with the payload
                let tx = match tx.get(&payload.get_type()) {
                    Some(sender) => sender.clone(),
                    None => {
                        log::error!("No sender found for event type : {:?}", payload.get_type());
                        return B::default();
                    }
                };
                payload.do_something(tx)
            }
        }
    }
}
