use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Copper<T> {
    ToClient {
        client_id: String,
        payload: T,
    },
    ToServer {
        client_id: String,
        callback_address: String,
        payload: T,
    },
}

impl<T> Copper<T> {
    pub fn to_client(client_id: String, payload: T) -> Self {
        Copper::ToClient { client_id, payload }
    }

    pub fn to_server(client_id: String, callback_address: String, payload: T) -> Self {
        Copper::ToServer {
            client_id,
            callback_address,
            payload,
        }
    }

    pub fn from_json(json_str: &str) -> serde_json::Result<Self>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_str(json_str)
    }

    pub fn to_json(&self) -> serde_json::Result<String>
    where
        T: Serialize,
    {
        serde_json::to_string(self)
    }

    pub fn client_id(&self) -> String {
        match self {
            Copper::ToClient { client_id, .. } => client_id.clone(),
            Copper::ToServer { client_id, .. } => client_id.clone(),
        }
    }

    pub fn callback_address(&self) -> String {
        match self {
            Copper::ToClient { .. } => "".into(),
            Copper::ToServer {
                callback_address, ..
            } => callback_address.clone(),
        }
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Copper<U> {
        match self {
            Copper::ToClient { client_id, payload } => Copper::ToClient {
                client_id,
                payload: f(payload),
            },
            Copper::ToServer {
                client_id,
                callback_address,
                payload,
            } => Copper::ToServer {
                client_id,
                callback_address,
                payload: f(payload),
            },
        }
    }
}
