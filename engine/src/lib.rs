mod calculation;
pub mod controllers;
mod lexer;
pub mod parser;
mod utils;

// Import Graph type
use parser::Graph;

use std::collections::HashMap;

use parser::{parse, Frame, Query};
use polars::frame::DataFrame;
use std::fs;

// use crate::controllers::fundamentals::FundamentalsController;
use crate::controllers::historical::HistoricalController;
// use crate::controllers::live::LiveController;
use crate::parser::{ModelType, Trades};

use crate::controllers::Output;

#[derive(Debug, Clone)]
pub enum EngineStatus {
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone)]
struct CodeDiff {
    frames: bool,
    graph: bool,
    trades: bool,
}

impl CodeDiff {
    pub fn new() -> Self {
        CodeDiff {
            frames: false,
            graph: false,
            trades: false,
        }
    }
}

#[derive(Debug)]
pub struct Engine {
    file_path: String,
    query: Query,
    status: EngineStatus,
    frames: HashMap<String, DataFrame>,
    code_diff: Option<CodeDiff>,
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
                frames: HashMap::new(),
                code_diff: None,
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

    pub async fn update_code(&mut self) -> Result<Output, String> {
        let code = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        match parse(&code) {
            Ok(query) => {
                self.code_diff = Some(CodeDiff {
                    frames: self.query.frame != query.frame,
                    graph: self.query.graph != query.graph,
                    trades: self.query.trade != query.trade,
                });

                self.query = query;

                self.run().await
            }
            Err(e) => Err(format!(
                "Failed to parse updated code: {}, line {}, column {}",
                e.message, e.line, e.column
            )),
        }
    }

    pub async fn restart(&mut self) -> Result<Output, String> {
        self.status = EngineStatus::Stopped;
        self.code_diff = None;

        self.run().await
    }

    pub async fn run(&mut self) -> Result<Output, String> {
        // first iter through models in frame and pull data,
        // this will also handle actions on models

        self.status = EngineStatus::Running;

        if let Err(e) = self.analyze() {
            self.status = EngineStatus::Error(e.clone());
            return Err(e);
        }

        let cd = self.code_diff.clone().unwrap_or(CodeDiff::new());

        log::info!("Running engine for file: {}", self.file_path);
        log::info!("Code diff: {:?}", cd);

        let mut result_df = |name: String, r: Result<DataFrame, String>| match r {
            Ok(df) => {
                self.frames.insert(name.clone(), df);
                self.status = EngineStatus::Stopped;
                // Ok(())
            }
            Err(e) => {
                self.status = EngineStatus::Error(e.clone());
                log::error!("Failed to execute model: {}", e);
                // return Err(format!("Failed to execute model: {}", e));
            }
        };

        let mut model_gate =
            async |name: String, frame: &Frame, model_type: ModelType| match model_type {
                ModelType::Live => {
                    log::info!("Live model type is not supported yet");
                }
                ModelType::Historical => {
                    let controller = HistoricalController::new(frame, None);
                    let df = controller.execute().await;
                    result_df(name.clone(), df);
                }
                ModelType::Fundamental => {
                    log::info!("Fundamental model type is not supported yet");
                }
            };

        if !cd.frames {
            for (name, frame) in self.query.frame.iter() {
                model_gate(name.clone(), frame, frame.model.model_type.clone()).await;
            }
        }

        let mut graph: Option<Graph> = None;
        let mut trades: Option<DataFrame> = None;

        // Now this will check if it needs to build a graph
        if let Some(g) = &self.query.graph {
            graph = match utils::graph::graph_over_data(g, &self.frames) {
                Ok(g) => Some(g),
                Err(e) => {
                    log::error!("Failed to build graph: {}", e);
                    return Err(format!("Failed to build graph: {}", e));
                }
            };
        }

        if let Some(trade_section) = &self.query.trade {
            log::info!("Building trades over data");
            trades = match utils::trade::trades_over_data(trade_section, &self.frames) {
                Ok(df) => Some(df),
                Err(e) => {
                    log::error!("Failed to build trades: {}", e);
                    return Err(format!("Failed to build trades: {}", e));
                }
            };
        } else {
            log::info!("No trade section found in query");
        }

        self.status = EngineStatus::Stopped;
        self.code_diff = None;

        log::info!("Engine run completed for file: {}", self.file_path);
        if let Some(_) = &graph {
            log::info!("Graph Generated");
        } else {
            log::info!("No graph generated");
        }
        log::info!("Generated {} Tables", self.frames.len());
        if let Some(trades) = &trades {
            log::info!("Trades: {} rows", trades.height());
        } else {
            log::info!("No trades generated");
        }

        let mut t: Option<Trades> = None;
        if let Some(trades_df) = trades {
            let over_frame = self.query.trade.as_ref().unwrap().over_frame.clone();
            let over_frame_df = self
                .frames
                .get(&over_frame)
                .ok_or_else(|| format!("Frame '{}' not found for trades", over_frame))?;
            t = Some(Trades {
                trades_table: trades_df.clone(),
                trades_graph: utils::trade::trade_graphing_util(
                    self.query.trade.clone().unwrap(),
                    &trades_df,
                    &over_frame_df,
                ),
                over_frame,
            });
        }

        Ok(Output::Data {
            graph,
            tables: self.frames.clone(),
            trades: t,
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
