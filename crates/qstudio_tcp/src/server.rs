use busbar::{Copper, MakeT, Response, Unravel};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::thread;

use actix_rt;

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

    pub fn listen<A, T: Unravel<A, T, B>, B: Response<A, B>, C>(
        &self,
        tx: HashMap<A, Sender<(C, T)>>,
        client_list: Arc<Mutex<ClientList<C>>>,
    ) where
        T: for<'de> serde::Deserialize<'de> + std::fmt::Display,
        A: std::hash::Hash + Eq + Clone + std::fmt::Debug,
        B: MakeT<T>,
        C: From<String> + std::fmt::Debug + Clone,
    {
        let listener = TcpListener::bind(&self.rx_addr).unwrap();
        log::info!("Server listening on {}", self.rx_addr);
        // self.http();

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    log::info!("TCP STREAM STARTED : {}", stream.peer_addr().unwrap());
                    for (client, b) in
                        self.incoming::<A, T, B, C>(stream, &tx, Arc::clone(&client_list))
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

    fn incoming<A, T, B, C>(
        &self,
        stream: TcpStream,
        tx: &HashMap<A, Sender<(C, T)>>,
        client_list: Arc<Mutex<ClientList<C>>>,
    ) -> Vec<(C, B)>
    where
        A: std::hash::Hash + Eq + Clone + std::fmt::Debug,
        T: for<'de> serde::Deserialize<'de> + std::fmt::Display + Unravel<A, T, B>,
        B: Response<A, B>,
        C: From<String> + std::fmt::Debug + Clone,
    {
        let addr = stream.peer_addr().unwrap();
        log::info!("Client connected: {}", addr);

        let mut res = Vec::new();

        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(message) => {
                    let copper: Copper<T> = Copper::from_json(&message).unwrap();
                    match &copper {
                        Copper::RemoveClient {
                            client_id,
                            callback_address,
                        } => {
                            log::info!("Removing client with id: {}", client_id);
                            client_list.lock().unwrap().remove_client(client_id);
                            continue;
                        }
                        Copper::ToServer {
                            client_id,
                            callback_address,
                            payload,
                        } => {
                            log::info!("Received from client {}: {}", client_id, payload);
                            client_list.lock().unwrap().add_client(
                                client_id.to_string(),
                                C::from(callback_address.clone()),
                            );

                            log::info!("Client List: {:#?}", client_list.lock().unwrap().clients);

                            res.push(self.handle_message(copper, tx, Arc::clone(&client_list)));
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    log::error!("Error reading from client {}: {}", addr, e);
                    return vec![(C::from("".into()), B::default())];
                }
            }
        }
        res
    }

    fn handle_message<A, T: Unravel<A, T, B>, B: Response<A, B>, C>(
        &self,
        message: Copper<T>,
        tx: &HashMap<A, Sender<(C, T)>>,
        client_list: Arc<Mutex<ClientList<C>>>,
    ) -> (C, B)
    where
        A: Eq + std::hash::Hash + Clone + std::fmt::Debug,
        B: Response<A, B>,
        C: From<String> + Clone + std::fmt::Debug,
    {
        match message {
            Copper::ToClient { .. } => return (C::from("".into()), B::default()),
            Copper::RemoveClient {
                client_id,
                callback_address,
            } => {
                log::info!("Removing client with id: {}", client_id);
                client_list.lock().unwrap().remove_client(&client_id);
                return (C::from("".into()), B::default());
            }
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
                    client_list.add_client(client_id.clone(), C::from(callback_address.clone()));
                }
                let tx = match tx.get(&payload.get_type()) {
                    Some(sender) => sender.clone(),
                    None => {
                        log::error!("No sender found for event type : {:?}", payload.get_type());
                        return (C::from("".into()), B::default());
                    }
                };
                let client = match client_list.get_client(&client_id) {
                    Some(c) => c.clone(),
                    None => {
                        log::error!("Client not found after adding: {}", client_id);
                        return (C::from("".into()), B::default());
                    }
                };
                (client.clone(), payload.do_something(tx, client.clone()))
            }
        }
    }
}
