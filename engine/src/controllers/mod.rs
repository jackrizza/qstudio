use polars::frame::DataFrame;
use std::collections::HashMap;

pub mod fundamentals;
pub mod historical;
pub mod live;

use crate::parser::Graph;

#[derive(Debug)]
pub enum Output {
    Data {
        graph: Option<Graph>,
        tables: HashMap<String, DataFrame>,
        trades: Option<DataFrame>,
    },
    Error(String),
    None,
}

impl Output {
    pub fn get_graph(&self) -> Option<&Graph> {
        match self {
            Output::Data { graph, .. } => graph.as_ref(),
            _ => None,
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
