use std::sync::{Arc, Mutex};
use std::thread;

pub struct Tick(Arc<Mutex<i64>>);

impl Tick {
    pub fn new(value: i64) -> Self {
        Tick(Arc::new(Mutex::new(value)))
    }

    pub fn get(&self) -> i64 {
        *self.0.lock().unwrap()
    }

    pub fn tick_thread(&self) {
        let tick = Arc::clone(&self.0);
        thread::spawn(move || loop {
            thread::sleep(std::time::Duration::from_millis(1000));
            let mut tick_value = tick.lock().unwrap();
            *tick_value += 1;
        });
    }
}
