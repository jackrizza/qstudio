use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub mod models;
pub mod views;
pub mod utils;

use models::engine::EngineEvent;
use models::notification::Notification;
use models::ui::UIEvent;

pub struct Channels {
    pub ui_rx: Receiver<UIEvent>,
    pub ui_tx: Arc<Mutex<Sender<UIEvent>>>,

    pub notification_tx: Arc<Mutex<Sender<Notification>>>,

    pub engine_rx: Arc<Receiver<EngineEvent>>,
    pub engine_tx: Arc<Mutex<Sender<EngineEvent>>>,
}
