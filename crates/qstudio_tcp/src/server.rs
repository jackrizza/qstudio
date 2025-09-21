use busbar::{Copper, MakeT, Response, Unravel};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};

use crate::{client, Client, ClientList};
use std::sync::{Arc, Mutex};

pub struct Server {
    pub rx_addr: String,
    pub tx_addr: String,
}

impl Server {
    pub fn new(rx_addr: String, tx_addr: String) -> Self {
        Server { rx_addr, tx_addr }
    }

    pub fn listen<A, T: Unravel<A, T, B>, B: Response<A, B>>(
        &self,
        tx: HashMap<A, Sender<(Client, T)>>,
        client_list: Arc<Mutex<ClientList>>,
    ) where
        T: for<'de> serde::Deserialize<'de> + std::fmt::Display,
        A: std::hash::Hash + Eq + Clone + std::fmt::Debug,
        B: MakeT<T>,
    {
        let listener = TcpListener::bind(&self.rx_addr).unwrap();
        log::info!("Server listening on {}", self.rx_addr);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    log::info!("TCP STREAM STARTED : {}", stream.peer_addr().unwrap());
                    for (client, b) in
                        self.incoming::<A, T, B>(stream, &tx, Arc::clone(&client_list))
                    {
                        log::info!("Response: {}", b.message());

                        log::info!("Found sender for event type: {:?}", b.event_type());
                        if let Some(sender) = tx.get(&b.event_type()) {
                            let event = b.make_t();

                            if let Err(e) = sender.send((client, event)) {
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

    fn incoming<A, T, B>(
        &self,
        stream: TcpStream,
        tx: &HashMap<A, Sender<(Client, T)>>,
        client_list: Arc<Mutex<ClientList>>,
    ) -> Vec<(Client, B)>
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

                    client_list.lock().unwrap().add_client(
                        copper.client_id().to_string(),
                        copper.callback_address().to_string(),
                    );

                    log::info!("Client List: {:#?}", client_list.lock().unwrap().clients);

                    res.push(self.handle_message(copper, tx, Arc::clone(&client_list)));
                }
                Err(e) => {
                    log::error!("Error reading from client {}: {}", addr, e);
                    return vec![(Client::new("".into()), B::default())];
                }
            }
        }
        res
    }

    fn handle_message<A, T: Unravel<A, T, B>, B: Response<A, B>>(
        &self,
        message: Copper<T>,
        tx: &HashMap<A, Sender<(Client, T)>>,
        client_list: Arc<Mutex<ClientList>>,
    ) -> (Client, B)
    where
        A: Eq + std::hash::Hash + Clone + std::fmt::Debug,
        B: Response<A, B>,
    {
        match message {
            Copper::ToClient { .. } => return (Client::new("".into()), B::default()),
            Copper::ToServer {
                client_id,
                callback_address,
                payload,
            } => {
                // do something with the payload
                log::info!("client for this {}, {}", client_id, callback_address);
                let mut client_list = client_list.lock().unwrap();
                if let None = client_list.get_client(&client_id) {
                    log::info!("Registering new client with id: {}", client_id);
                    client_list.add_client(client_id.clone(), callback_address);
                }
                let tx = match tx.get(&payload.get_type()) {
                    Some(sender) => sender.clone(),
                    None => {
                        log::error!("No sender found for event type : {:?}", payload.get_type());
                        return (Client::new("".into()), B::default());
                    }
                };
                let client = match client_list.get_client(&client_id) {
                    Some(c) => c.clone(),
                    None => {
                        log::error!("Client not found after adding: {}", client_id);
                        return (Client::new("".into()), B::default());
                    }
                };
                (client.clone(), payload.do_something(tx, client.clone()))
            }
        }
    }
}
