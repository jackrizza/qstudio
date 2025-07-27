mod calculation;
pub mod controllers;
mod lexer;
pub mod parser;

use parser::{parse, Query};
use polars::frame::DataFrame;

use crate::controllers::fundamentals::FundamentalsController;
use crate::controllers::historical::HistoricalController;
use crate::controllers::live::LiveController;
use crate::parser::ModelType;

use crate::controllers::Output;

#[derive(Debug)]
pub struct Engine(Query);

fn remove_comments(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim().starts_with("--"))
        // .map(|line| line.trim())
        .collect::<Vec<&str>>()
        .join(" ")
}

impl Engine {
    pub fn new(token_stream: &str) -> Result<Self, String> {
        let stripped = remove_comments(token_stream);
        match parse(&stripped) {
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
        match self.0.model.model_type {
            ModelType::Live => Err("Live queries not implemented".to_string()),
            ModelType::Historical => HistoricalController::new(&self.0).execute().await,
            ModelType::Fundamental => Err("Fundamental queries not implemented".to_string()),
            _ => Err("Unsupported model type".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;

    #[tokio::test]
    async fn test_engine() {
        let src = r#"
            HISTORICAL TICKER aapl 
            FROM 20220101 TO 20221231
            PULL open, high, low, close
            CALC open,close DIFFERENCE CALLED diff_field
            SHOWTABLE
        "#;

        let mut engine = Engine::new(&src.replace("\n", " ")).unwrap();
        let result = engine.run().await;
        // println!("{:#?}", result);
        assert!(result.is_ok());
    }
}
