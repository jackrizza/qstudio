use polars::frame::DataFrame;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
pub mod fundamentals;
pub mod historical;
pub mod live;

use crate::parser::{Graph, Trades};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Output {
    Pending,
    Data {
        graph: Option<Graph>,
        tables: HashMap<String, DataFrame>,
        trades: Option<Trades>,
    },
    Error(String),
    None,
}

#[derive(Debug, Clone)]
pub enum FireAndForget {
    Pending,
    Done(DataFrame),
    Failed(String),
}

impl FireAndForget {
    pub fn is_pending(&self) -> bool {
        matches!(self, FireAndForget::Pending)
    }

    pub fn is_done(&self) -> bool {
        matches!(self, FireAndForget::Done(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, FireAndForget::Failed(_))
    }
}

impl Output {
    pub fn get_graph(&self) -> Option<&Graph> {
        match self {
            Output::Data { graph, .. } => graph.as_ref(),
            _ => None,
        }
    }

    pub fn get_trades(&self) -> Option<Trades> {
        match self {
            Output::Data { trades, .. } => {
                if trades.is_none() {
                    log::warn!("No trades available");
                }
                trades.clone()
            }
            _ => {
                log::warn!("No trades available");
                None
            }
        }
    }

    pub fn get_tables(&self) -> Option<&HashMap<String, DataFrame>> {
        match self {
            Output::Data { tables, .. } => Some(tables),
            _ => None,
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        Output::None
    }
}
