mod client;
mod server;

pub use client::Client;
pub use server::Server;

use std::collections::HashMap;

pub struct ClientList {
    clients: HashMap<String, Client>, // Map client IDs to their addresses
}

impl ClientList {
    pub fn new() -> Self {
        ClientList {
            clients: HashMap::new(),
        }
    }

    pub fn add_client(&mut self, client_id: String, address: String) {
        let client = Client::new(address);
        self.clients.insert(client_id, client);
    }

    pub fn get_client(&self, client_id: &str) -> Option<Client> {
        self.clients.get(client_id).cloned()
    }

    pub fn remove_client(&mut self, client_id: &str) {
        self.clients.remove(client_id);
    }
}
