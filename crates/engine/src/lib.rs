mod calculation;
mod lexer;
pub mod parser;
pub mod runtime;
pub mod utils;

// Import Graph type
use parser::Graph;

use std::collections::HashMap;

use parser::{parse, Query};
use polars::frame::DataFrame;
use serde_json::Value;
use std::fs;

use polars::prelude::*;
use std::io::Cursor;

use crate::parser::Trades;

pub mod output;
use crate::output::Output;

use provider::{
    models::Entity,
    tcp::client::client::{Client, ClientBuilder},
};

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
    provider_frames: HashMap<String, DataFrame>,
    frames: HashMap<String, DataFrame>,

    providers: Client,

    // code_diff: Option<CodeDiff>,
    output: Option<Output>,
    new_output: bool,
    _for_test_flag: bool,
    rt: runtime::GpuRuntime,
}

impl Engine {
    pub fn new(
        file_path: &str,
        provider_addr: &str,
        is_src_input: Option<bool>,
    ) -> Result<Self, String> {
        let is_src_input = is_src_input.unwrap_or(false);
        // let stripped = remove_comments(token_stream);

        let mut token_stream = String::new();
        if is_src_input {
            token_stream = file_path.to_string();
        } else {
            token_stream =
                fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
        }

        let providers = ClientBuilder::new(provider_addr)
            .connect()
            .map_err(|e| format!("Failed to connect to provider: {}", e))?;

        let rt = match pollster::block_on(async move {
            runtime::GpuRuntime::new().await.map_err(|e| e.to_string())
        }) {
            Ok(rt) => rt,
            Err(e) => return Err(e),
        };

        match parse(&token_stream) {
            Ok(query) => Ok(Engine {
                file_path: file_path.to_string(),
                query,
                status: EngineStatus::Stopped,

                provider_frames: HashMap::new(),
                frames: HashMap::new(),

                providers,

                // code_diff: None,
                output: None,
                new_output: false,
                _for_test_flag: is_src_input,
                rt,
            }),
            Err(e) => {
                return Err(format!(
                    "Failed to parse query: {}, line {}, column {}",
                    e.message, e.line, e.column
                ))
            }
        }
    }

    pub fn output_changed(&mut self) -> bool {
        if self.output.is_none() {
            return false;
        }
        let ret = self.new_output;
        self.new_output = false;
        ret
    }

    pub fn get_output(&self) -> Option<Output> {
        self.output.clone()
    }

    pub fn status(&self) -> &EngineStatus {
        &self.status
    }

    pub fn query(&self) -> &Query {
        &self.query
    }

    pub fn analyze(&self) -> Result<(), String> {
        // Analyze the query and return an error if it fails
        let mut file = String::new();

        if self._for_test_flag {
            file = self.file_path.to_string();
        } else {
            file = fs::read_to_string(&self.file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;
        }
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
                self.run()
            }
            Err(e) => Err(format!(
                "Failed to parse updated code: {}, line {}, column {}",
                e.message, e.line, e.column
            )),
        }
    }

    pub async fn restart(&mut self) -> Result<(), String> {
        self.status = EngineStatus::Stopped;

        self.run()
    }

    pub fn run(&mut self) -> Result<(), String> {
        // first iter through models in frame and pull data,
        // this will also handle actions on models

        self.status = EngineStatus::Running;

        if let Err(e) = self.analyze() {
            self.status = EngineStatus::Error(e.clone());
            return Err(format!("Failed to analyze code: {:?}", e));
        }

        log::info!("Running engine for file: {}", self.file_path);

        let mut queries = match self.query.build_provider_queries() {
            Ok(queries) => queries,
            Err(e) => {
                self.status = EngineStatus::Error(format!("Failed to build queries: {:?}", e));
                log::error!("Failed to build queries: {:?}", e);
                return Err(format!("Failed to build queries: {:?}", e));
            }
        };

        let get_data_result = match self.providers.get_data::<Vec<Entity>>(queries.clone()) {
            Ok(data) => data,
            Err(e) => {
                self.status = EngineStatus::Error(format!("Failed to get data: {}", e));
                log::error!("Failed to get data: {} for {:#?}", e, queries);
                return Err(format!("Failed to get data: {} for {:#?}", e, queries));
            }
        };

        for (name, entity) in get_data_result {
            let data: Vec<Value> = match serde_json::from_str(&entity[0].data) {
                Ok(data) => data,
                Err(e) => {
                    self.status = EngineStatus::Error(format!("Failed to deserialize data: {}", e));
                    log::error!(
                        "Failed to deserialize data: {} for {:#?}",
                        e,
                        entity[0].data
                    );
                    return Err(format!(
                        "Failed to deserialize data: {} for {:#?}",
                        e, entity[0].data
                    ));
                }
            };
            let df = match json_values_to_df(&data) {
                Ok(df) => df,
                Err(e) => {
                    self.status = EngineStatus::Error(format!("Failed to deserialize data: {}", e));
                    log::error!("Failed to deserialize data: {} for {:#?}", e, data);
                    return Err(format!("Failed to deserialize data: {} for {:#?}", e, data));
                }
            };

            self.provider_frames.insert(name, df);
        }

        for (name, frame) in self.query.frame.iter() {
            let p = match self.provider_frames.get(&frame.provider) {
                Some(provider) => provider,
                None => {
                    return Err(format!(
                        "Provider : {} not found for frame: {}",
                        frame.provider, name
                    ))
                }
            };

            // let provider = match action_over_data(&frame.actions, p.clone()) {
            //     Ok(provider) => provider,
            //     Err(e) => {
            //         log::error!("Failed to apply actions for frame: {}", e);
            //         return Err(format!("Failed to apply actions for frame: {}", e));
            //     }
            // };

            println!("Time for th gpu");
            let provider = match utils::action::action_over_data_gpu(
                &frame.actions,
                p.clone(),
                &mut self.rt,
            ) {
                Ok(provider) => provider,
                Err(e) => {
                    log::error!("Failed to apply actions for frame: {}", e);
                    return Err(format!("Failed to apply actions for frame: {}", e));
                }
            };

            println!("Adding provider_frame: {}", provider.head(Some(40)));
            self.frames.insert(name.clone(), provider.clone());
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
        // self.code_diff = None;

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
            log::info!("Trades: {:?}", trades_df);
            t = Some(Trades {
                trades_table: trades_df.clone(),
                trades_graph: utils::trade::trade_graphing_util(
                    self.query.trade.clone().unwrap(),
                    &trades_df,
                    &over_frame_df,
                ),
                trade_summary: utils::trade::trade_summary_util(
                    self.query.trade.clone().unwrap(),
                    &trades_df,
                    &over_frame_df,
                ),
                over_frame,
            });
        }

        self.output = Some(Output::Data {
            graph,
            tables: self.frames.clone(),
            trades: t,
        });

        self.new_output = true;

        Ok(())
    }
}

pub fn json_values_to_df(values: &[Value]) -> std::io::Result<DataFrame> {
    // values should be an array of JSON objects (or nested; Polars can hold Struct/Lists)
    let bytes = serde_json::to_vec(values)?;
    let df = JsonReader::new(Cursor::new(bytes))
        // Use `JsonFormat::Json` for a JSON array; use `JsonFormat::JsonLines` for NDJSON
        .with_json_format(JsonFormat::Json)
        // .infer_schema_len(Some(1_000)) // optional: how many rows to scan for schema
        .finish()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(df)
}

#[cfg(test)]
mod tests {

    use super::Engine;

    #[test]
    fn test_engine() {
        let src = r#"

            PROVIDER aapl_data
                PROVIDER yahoo_finance
                TICKER aapl
                FROM 20200101 TO 20250901

            FRAME test
                PROVIDER aapl_data
                PULL open, high, low, close
                CALC open, close DIFFERENCE CALLED diff_field

            TRADE
                STOCK
                ENTRY test.open, test.close, 0.5
                EXIT test.high, test.low, 0.5
                LIMIT 0.1
                HOLD 14
        "#;

        let mut engine = Engine::new(&src, "127.0.0.1:7000", Some(true)).unwrap();
        println!("Engine : {:#?}", engine.query());

        match engine.run() {
            Ok(_) => {
                println!("Engine run successfully");
                assert_eq!(engine.frames.get("test").is_some(), true);
            }
            Err(e) => {
                println!("Engine run failed: {}", e);
                assert_eq!(false, true);
            }
        }
    }
}
