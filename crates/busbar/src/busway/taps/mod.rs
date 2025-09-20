// tap is a a io operation
// each tap can be a different type of io operation
// but will return a common type

use polars::prelude::*;

use crate::busway::{Battery, Filters};

pub enum Tap {
    YahooFinance { battery: Option<String> },
    Sec { battery: Option<String> },
}

impl Tap {
    pub fn battery_uid(&self) -> Option<String> {
        match self {
            Tap::YahooFinance { battery } => battery.clone(),
            Tap::Sec { battery } => battery.clone(),
        }
    }
}

impl super::Rectifier for Tap {
    fn refresh(&mut self) {
        // refresh the tap
    }

    fn get_data(&self, battery: &mut Battery, filters: Vec<Filters>) -> DataFrame {
        // return the data from the tap
        battery.get_data(&filters)
    }
}
