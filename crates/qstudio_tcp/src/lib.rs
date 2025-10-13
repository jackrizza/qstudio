mod client;
mod server;

pub use client::Client;
pub use server::Server;

use std::collections::HashMap;

pub struct ClientList<T> {
    clients: HashMap<String, T>, // Map client IDs to their addresses
}

impl<T> ClientList<T> {
    pub fn new() -> Self {
        ClientList {
            clients: HashMap::new(),
        }
    }

    pub fn add_client(&mut self, client_id: String, t: T) {
        self.clients.insert(client_id, t);
    }

    pub fn get_client(&self, client_id: &str) -> Option<T>
    where
        T: Clone,
    {
        self.clients.get(client_id).cloned()
    }

    pub fn remove_client(&mut self, client_id: &str) {
        self.clients.remove(client_id);
    }
}
