mod calculation;
pub mod controllers;
mod lexer;
pub mod parser;
mod utils;

// Import Graph type
use parser::Graph;

use std::collections::HashMap;

use parser::{parse, Query};
use polars::frame::DataFrame;

use crate::controllers::fundamentals::FundamentalsController;
use crate::controllers::historical::HistoricalController;
use crate::controllers::live::LiveController;
use crate::parser::ModelType;

use crate::controllers::Output;

#[derive(Debug)]
pub struct Engine(Query);

impl Engine {
    pub fn new(token_stream: &str) -> Result<Self, String> {
        // let stripped = remove_comments(token_stream);
        match parse(&token_stream) {
            Ok(query) => Ok(Engine(query)),
            Err(e) => {
                return Err(format!(
                    "Failed to parse query: {}, line {}, column {}",
                    e.message, e.line, e.column
                ))
            }
        }
    }

    pub fn query(&self) -> &Query {
        &self.0
    }

    pub async fn run(&mut self) -> Result<Output, String> {
        // first iter through models in frame and pull data,
        // this will also handle actions on models

        let mut frames: HashMap<String, DataFrame> = HashMap::new();

        for (name, frame) in self.0.frame.iter() {
            match frame.model.model_type {
                ModelType::Live => {
                    return Err("Live model type is not supported yet".to_string());
                }
                ModelType::Historical => {
                    let controller = HistoricalController::new(frame, None);
                    let df = controller.execute().await?;
                    frames.insert(name.clone(), df);
                }

                ModelType::Fundamental => {
                    return Err("Fundamental model type is not supported yet".to_string());
                }
            }
        }

        let mut graph: Option<Graph> = None;

        // Now this will check if it needs to build a graph
        if let Some(g) = &self.0.graph {
            graph = match utils::graph::graph_over_data(g, &frames) {
                Ok(g) => Some(g),
                Err(e) => return Err(format!("Failed to build graph: {}", e)),
            }
        }

        Ok(Output::Data {
            graph,
            tables: frames,
            trades: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;

    #[tokio::test]
    async fn test_engine() {
        let src = r#"
            FRAME test
                HISTORICAL TICKER aapl 
                FROM 20220101 TO 20221231
                PULL open, high, low, close
                CALC open,close DIFFERENCE CALLED diff_field
        "#;

        let mut engine = Engine::new(&src.replace("\n", " ")).unwrap();

        println!("Running engine with query: {:#?}", engine.query());

        let result = engine.run().await;
        // println!("{:#?}", result);
        assert!(result.is_ok());
    }
}
