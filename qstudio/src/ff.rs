use crate::{Receivers, Senders};
use std::sync::Arc;
use tokio::runtime::{Builder, Handle, Runtime};

pub struct Channels {
    pub senders: Arc<Senders>,
    pub receivers: Arc<Receivers>,
}

// Add this to whatever top-level state owns your threads:
pub struct AsyncExec {
    _rt: Arc<Runtime>, // keep the runtime alive
    handle: Handle,    // used to spawn from non-async threads
}

impl AsyncExec {
    pub fn new() -> Arc<Self> {
        let rt = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        Arc::new(Self {
            handle: rt.handle().clone(),
            _rt: Arc::new(rt),
        })
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }
}
