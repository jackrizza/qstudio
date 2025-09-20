use std::collections::HashMap;

/// Busway
/// A busway is a collection of requests to various data sources, which are then
/// processed and returned as a single response.
///
///
use polars::prelude::*;

mod battery;
mod taps;
use taps::Tap;

use battery::Battery;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    YahooFinance,
    Sec,
}

pub enum Filters {
    DateRange { start: i64, end: i64 },
    Frequency(String),
}

pub trait Rectifier {
    fn refresh(&mut self);
    fn get_data(&self, battery: &mut Battery, filters: Vec<Filters>) -> DataFrame;
}

pub struct Busway {
    // <uid, battery>
    bank: HashMap<String, Battery>,
    // <symbol, tap>
    taps: HashMap<String, Vec<Tap>>,
}

impl Busway {
    pub fn new() -> Self {
        Self {
            taps: HashMap::new(),
            bank: HashMap::new(),
        }
    }

    pub fn add_tap(&mut self, symbol: String, sources: Vec<Source>) {
        let mut tap_vec = Vec::new();
        for source in sources {
            match source {
                Source::YahooFinance => {
                    // create a new battery for this tap
                    let battery = Battery::new(symbol.clone(), source);
                    tap_vec.push(Tap::YahooFinance {
                        battery: Some(battery.uid.clone()),
                    });
                    self.bank.insert(battery.uid.clone(), battery);
                }
                Source::Sec => {
                    let battery = Battery::new(symbol.clone(), source);
                    tap_vec.push(Tap::Sec {
                        battery: Some(battery.uid.clone()),
                    });
                    self.bank.insert(battery.uid.clone(), battery);
                }
            }
        }
        self.taps.insert(symbol, tap_vec);
    }

    pub fn get_data(
        &mut self,
        symbol: String,
        filters: Vec<Filters>,
        source: Source,
    ) -> Result<DataFrame, String> {
        let taps = match self.taps.get(&symbol) {
            Some(taps) => taps,
            None => return Err(format!("No taps found for symbol: {}", symbol)),
        };
        let tap = match source {
            Source::YahooFinance => taps.iter().find(|t| matches!(t, Tap::YahooFinance { .. })),
            Source::Sec => taps.iter().find(|t| matches!(t, Tap::Sec { .. })),
        };

        match tap {
            Some(tap) => {
                let mut battery = match tap.battery_uid() {
                    Some(uid) => match self.bank.get_mut(&uid) {
                        Some(battery) => battery,
                        None => return Err("Battery UID not found in bank".to_string()),
                    },
                    None => return Err("No battery associated with this tap".to_string()),
                };
                Ok(tap.get_data(&mut battery, filters))
            }
            None => Err(format!(
                "No tap found for symbol: {} and source: {:?}",
                symbol, source
            )),
        }
    }

    pub fn refresh(&mut self) {
        // Refresh all taps
        for tap in self.taps.values_mut() {
            for t in tap {
                t.refresh();
            }
        }
    }
}
