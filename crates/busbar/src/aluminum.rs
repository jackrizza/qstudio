use core::fmt;
use crossbeam_channel::{Receiver, Sender};

pub struct Aluminum<T: Send + 'static> {
    pub backend_rx: Receiver<T>,
    pub backend_tx: Sender<T>,

    pub frontend_rx: Receiver<T>,
    pub frontend_tx: Sender<T>,

    pub notification_rx: Receiver<T>,
    pub notification_tx: Sender<T>,

    pub filetree_rx: Receiver<T>,
    pub filetree_tx: Sender<T>,

    pub dock_rx: Receiver<T>,
    pub dock_tx: Sender<T>,
    pub dock_backend_tx: Sender<T>,
    pub dock_backend_rx: Receiver<T>,

    pub engine_rx: Receiver<T>,
    pub engine_tx: Sender<T>,

    pub widget_rx: Receiver<T>,
    pub widget_tx: Sender<T>,
}

impl<T: Send + 'static> fmt::Debug for Aluminum<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Aluminum")
            .field("backend_rx", &"Receiver<Event>")
            .field("backend_tx", &"Sender<Event>")
            .field("frontend_rx", &"Receiver<Event>")
            .field("frontend_tx", &"Sender<Event>")
            .field("notification_rx", &"Receiver<Event>")
            .field("notification_tx", &"Sender<Event>")
            .field("filetree_rx", &"Receiver<Event>")
            .field("filetree_tx", &"Sender<Event>")
            .field("dock_rx", &"Receiver<Event>")
            .field("dock_tx", &"Sender<Event>")
            .field("dock_backend_rx", &"Receiver<Event>")
            .field("dock_backend_tx", &"Sender<Event>")
            .field("engine_rx", &"Receiver<Event>")
            .field("engine_tx", &"Sender<Event>")
            .field("widget_rx", &"Receiver<Event>")
            .field("widget_tx", &"Sender<Event>")
            .finish()
    }
}

impl<T: Send + 'static> Aluminum<T> {
    pub fn new() -> Self {
        let (backend_tx, backend_rx) = crossbeam_channel::unbounded();
        let (frontend_tx, frontend_rx) = crossbeam_channel::unbounded();
        let (notification_tx, notification_rx) = crossbeam_channel::unbounded();
        let (filetree_tx, filetree_rx) = crossbeam_channel::unbounded();
        let (dock_tx, dock_rx) = crossbeam_channel::unbounded();
        let (dock_backend_tx, dock_backend_rx) = crossbeam_channel::unbounded();
        let (engine_tx, engine_rx) = crossbeam_channel::unbounded();
        let (widget_tx, widget_rx) = crossbeam_channel::unbounded();

        Aluminum {
            backend_rx,
            backend_tx,
            frontend_rx,
            frontend_tx,
            notification_rx,
            notification_tx,
            filetree_rx,
            filetree_tx,
            dock_rx,
            dock_tx,
            dock_backend_tx,
            dock_backend_rx,
            engine_rx,
            engine_tx,
            widget_rx,
            widget_tx,
        }
    }

    pub fn backend_listen(&self) {
        let frontend_tx = self.frontend_tx.clone();

        loop {
            match self.backend_rx.recv() {
                Ok(message) => {
                    // Process the message and send a response if needed
                    // For demonstration, we just echo the message back
                    if let Err(e) = frontend_tx.send(message) {
                        log::error!("Error sending to frontend: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("Error receiving from backend: {}", e);
                    break;
                }
            }
        }
    }
}
