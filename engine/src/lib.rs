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
use std::fs;

use crate::controllers::fundamentals::FundamentalsController;
use crate::controllers::historical::HistoricalController;
use crate::controllers::live::LiveController;
use crate::parser::ModelType;

use crate::controllers::Output;

#[derive(Debug, Clone)]
pub enum EngineStatus {
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug)]
pub struct Engine {
    file_path: String,
    query: Query,
    status: EngineStatus,
}

impl Engine {
    pub fn new(file_path: &str) -> Result<Self, String> {
        // let stripped = remove_comments(token_stream);
        let token_stream =
            fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
        match parse(&token_stream) {
            Ok(query) => Ok(Engine {
                file_path: file_path.to_string(),
                query,
                status: EngineStatus::Stopped,
            }),
            Err(e) => {
                return Err(format!(
                    "Failed to parse query: {}, line {}, column {}",
                    e.message, e.line, e.column
                ))
            }
        }
    }

    pub fn status(&self) -> &EngineStatus {
        &self.status
    }

    pub fn query(&self) -> &Query {
        &self.query
    }

    pub fn analyze(&self) -> Result<(), String> {
        // Analyze the query and return an error if it fails
        let file = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        match parse(&file) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Failed to analyze query: {}, line {}, column {}",
                e.message, e.line, e.column
            )),
        }
    }

    pub fn update_code(&mut self) -> Result<(), String> {
        let code = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        match parse(&code) {
            Ok(query) => {
                self.query = query;
                Ok(())
            }
            Err(e) => Err(format!(
                "Failed to parse updated code: {}, line {}, column {}",
                e.message, e.line, e.column
            )),
        }
    }

    pub async fn run(&mut self) -> Result<Output, String> {
        // first iter through models in frame and pull data,
        // this will also handle actions on models

        let mut frames: HashMap<String, DataFrame> = HashMap::new();

        for (name, frame) in self.query.frame.iter() {
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
        let mut trades: Option<DataFrame> = None;

        // Now this will check if it needs to build a graph
        if let Some(g) = &self.query.graph {
            graph = match utils::graph::graph_over_data(g, &frames) {
                Ok(g) => Some(g),
                Err(e) => return Err(format!("Failed to build graph: {}", e)),
            }
        }

        if let Some(trade_section) = &self.query.trade {
            trades = match utils::trade::trades_over_data(trade_section, &frames) {
                Ok(df) => Some(df),
                Err(e) => {
                    return Err(format!("Failed to build trades: {}", e));
                }
            }
        }

        Ok(Output::Data {
            graph,
            tables: frames,
            trades,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::controllers::Output;
    use indoc::indoc;

    use super::Engine;

    #[tokio::test]
    async fn test_engine() -> Result<(), ()> {
        let src = indoc! {r#"
            FRAME test
                HISTORICAL 
                TICKER aapl 
                FROM 20220101 TO 20221231
                PULL open, high, low, close
                CALC open, close DIFFERENCE CALLED diff_field

            TRADE 
                STOCK
                ENTRY test.open, test.close, 0.5
                EXIT test.high, test.low, 0.5
                LIMIT 0.1
                HOLD 14
        "#};

        let mut engine = Engine::new(&src).unwrap();
        println!("Engine : {:#?}", engine.query());

        let result = match engine.run().await {
            Ok(output) => output,
            Err(e) => {
                println!("Error running engine: {}", e);
                return Err(());
            }
        };

        match result {
            Output::Data {
                graph: _,
                tables,
                trades,
            } => {
                if trades.is_none() {
                    return Err(());
                }
                println!("tables: {:#?}", tables);
                println!("trades: {:#?}", trades.unwrap());
            }
            _ => return Err(()),
        }

        // If we reach here, the engine ran successfully.
        Ok(())
    }
}
