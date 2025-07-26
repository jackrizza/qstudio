use super::*;

use polars::frame::DataFrame;
use polars::prelude::*;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::calculation::Calculation;
use crate::parser::ActionSection;
use crate::parser::GraphSection;
use crate::parser::ModelSection;
use crate::parser::Query;
use crate::parser::ShowType;
use crate::parser::TimeSpec;
use crate::parser::{DrawCommand, DrawType, Graph};
// Add a stub Graph struct so the type exists
pub struct HistoricalController<'a> {
    query: &'a Query,
}

impl<'a> HistoricalController<'a> {
    pub fn new(query: &'a Query) -> Self {
        HistoricalController { query }
    }

    pub async fn execute(&self) -> Result<Output, String> {
        // Implement the logic to handle historical queries
        let df = match pull_data(&self.query.model).await {
            Ok(data) => data,
            Err(e) => return Err(format!("Failed to pull data: {}", e)),
        };

        let action = action_over_data(&self.query.actions, df);

        if self.query.actions.show == ShowType::Table {
            match action {
                Ok(data_frame) => Ok(Output::DataFrame(data_frame)),
                Err(e) => Err(format!("Failed to process action: {}", e)),
            }
        } else if self.query.graph.is_none() {
            return Err("Graph section is missing".into());
        } else {
            let gs = self.query.graph.as_ref().unwrap();
            let graph = graph_over_data(gs, action?);
            let graph = match graph {
                Ok(g) => g,
                Err(e) => return Err(format!("Failed to create graph: {}", e)),
            };

            return Ok(Output::Graph(graph));
        }
    }
}

async fn pull_data(model: &ModelSection) -> Result<DataFrame, String> {
    let provider = yahoo::YahooConnector::new();

    // fallback ticker if your model doesn't yet provide one
    let ticker = model.ticker.as_str(); // assuming model has a `ticker: String` field

    // get the last 30 days of data
    let (from, to) = match model.time_spec {
        TimeSpec::DateRange { ref from, ref to } => {
            // Parse the start and end strings into OffsetDateTime
            // Convert "YYYYMMDD" to "YYYY-MM-DDT00:00:00Z" for RFC3339 parsing
            let from = &format!("{}-{}-{}T00:00:00Z", &from[0..4], &from[4..6], &from[6..8]);
            let to = &format!("{}-{}-{}T00:00:00Z", &to[0..4], &to[4..6], &to[6..8]);
            let start_dt =
                OffsetDateTime::parse(from, &time::format_description::well_known::Rfc3339)
                    .map_err(|e| format!("Failed to parse start date: {}", e))?;
            let end_dt = OffsetDateTime::parse(to, &time::format_description::well_known::Rfc3339)
                .map_err(|e| format!("Failed to parse end date: {}", e))?;
            (start_dt, end_dt)
        }
        _ => {
            // Default to the last 30 days if no time_spec is provided
            let now = OffsetDateTime::now_utc();
            (now - time::Duration::days(30), now)
        }
    };

    let provider = provider.map_err(|e| format!("Failed to create provider: {}", e))?;

    let response = provider
        .get_quote_history(ticker, from, to)
        .await
        .map_err(|e| format!("API error: {}", e))?;

    let quotes = response
        .quotes()
        .map_err(|e| format!("Failed to parse quotes: {}", e))?;

    if quotes.is_empty() {
        return Err("No data received".into());
    }

    // build columns
    let timestamps: Vec<_> = quotes.iter().map(|q| q.timestamp).collect();
    let closes: Vec<_> = quotes.iter().map(|q| q.close).collect();
    let opens: Vec<_> = quotes.iter().map(|q| q.open).collect();
    let highs: Vec<_> = quotes.iter().map(|q| q.high).collect();
    let lows: Vec<_> = quotes.iter().map(|q| q.low).collect();
    let volumes: Vec<_> = quotes.iter().map(|q| q.volume as u64).collect();

    // build polars DataFrame
    let df = df![
        "timestamp" => timestamps,
        "open" => opens,
        "high" => highs,
        "low" => lows,
        "close" => closes,
        "volume" => volumes
    ]
    .map_err(|e| format!("Failed to create DataFrame: {}", e))?;

    Ok(df)
}

fn action_over_data(action: &ActionSection, df: DataFrame) -> Result<DataFrame, String> {
    // Placeholder for actual action logic

    let field = action.fields.clone();

    let calc = match &action.calc {
        Some(calc) => calc,
        None => {
            let timestamps = df
                .column("timestamp")
                .map_err(|e| format!("Failed to get timestamp column: {}", e))?
                .clone();
            let selected_fields = df
                .select(field.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .map_err(|e| format!("Failed to select fields: {}", e))?;

            let mut columns = vec![timestamps];
            columns.extend_from_slice(selected_fields.get_columns());
            let concacted = DataFrame::new(columns)
                .map_err(|e| format!("Failed to create DataFrame: {}", e))?;
            return Ok(concacted); // No calculation, just return the concatenated fields
        } // No calculation, just return the DataFrame
    };

    let calculation = Calculation::new(calc.clone());
    let completed = calculation.calculate(&df)?;

    let timestamp = df
        .column("timestamp")
        .map_err(|e| format!("Failed to get timestamp column: {}", e))?
        .clone();

    // 2. Get selected fields from df (self.field)
    let selected_fields = df
        .select(field.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .map_err(|e| format!("Failed to select fields: {}", e))?;

    // 3. Concatenate timestamp, selected fields, and new_df
    let mut columns = vec![timestamp];
    columns.extend_from_slice(selected_fields.get_columns());
    columns.extend_from_slice(completed.get_columns());

    let result_df =
        DataFrame::new(columns).map_err(|e| format!("Failed to create DataFrame: {}", e))?;

    Ok(result_df)
}

fn graph_over_data(graph_section: &GraphSection, df: DataFrame) -> Result<Graph, String> {
    let timestamps = df
        .column("timestamp")
        .map_err(|e| format!("Missing 'timestamp' column: {}", e))?
        .i64()
        .map_err(|e| format!("Expected 'timestamp' to be f64: {}", e))?
        .to_vec();

    let axis_labels = timestamps
        .iter()
        .map(|ts| ts.unwrap_or(0).to_string())
        .collect::<Vec<_>>();

    let mut data: Vec<DrawType> = Vec::new();

    for command in &graph_section.commands {
        match command {
            DrawCommand::Line(fields) => {
                for field in fields {
                    let series = df
                        .column(field)
                        .map_err(|e| format!("Line column '{}' missing: {}", field, e))?
                        .f64()
                        .map_err(|e| format!("Line column '{}' not f64: {}", field, e))?;

                    let values = series
                        .to_vec()
                        .iter()
                        .map(|v| v.unwrap_or(0.0))
                        .collect::<Vec<_>>();
                    data.push(DrawType::Line(values));
                }
            }

            DrawCommand::Candle {
                open,
                high,
                low,
                close,
            } => {
                let open = df
                    .column(open)
                    .map_err(|e| e.to_string())?
                    .f64()
                    .map_err(|e| e.to_string())?
                    .to_vec();
                let high = df
                    .column(high)
                    .map_err(|e| e.to_string())?
                    .f64()
                    .map_err(|e| e.to_string())?
                    .to_vec();
                let low = df
                    .column(low)
                    .map_err(|e| e.to_string())?
                    .f64()
                    .map_err(|e| e.to_string())?
                    .to_vec();
                let close = df
                    .column(close)
                    .map_err(|e| e.to_string())?
                    .f64()
                    .map_err(|e| e.to_string())?
                    .to_vec();

                let candles: Vec<(f64, f64, f64, f64)> = open
                    .into_iter()
                    .zip(high)
                    .zip(low)
                    .zip(close)
                    .map(|(((o, h), l), c)| {
                        (
                            o.unwrap_or(0.0),
                            h.unwrap_or(0.0),
                            l.unwrap_or(0.0),
                            c.unwrap_or(0.0),
                        )
                    })
                    .collect();

                data.push(DrawType::Candlestick(candles));
            }

            DrawCommand::Bar(label) => {
                let values = df
                    .column(label)
                    .map_err(|e| format!("Bar column '{}' missing: {}", label, e))?
                    .f64()
                    .map_err(|e| format!("Bar column '{}' not f64: {}", label, e))?
                    .to_vec();

                let x: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
                data.push(DrawType::Bar(x));
            }
        }
    }

    Ok(Graph {
        data,
        axis_labels,
        title: "QQL Plot".into(),
    })
}
