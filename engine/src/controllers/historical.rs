use super::*;

use polars::frame::DataFrame;
use polars::prelude::*;
use time::OffsetDateTime;
use yahoo_finance_api as yahoo;

use crate::calculation::Calculation;
use crate::parser::ActionSection;
use crate::parser::Frame;
use crate::parser::ModelSection;
use crate::parser::TimeSpec;
use crate::parser::TradeSection;
use crate::parser::TradeType;
// Add a stub Graph struct so the type exists
pub struct HistoricalController<'a> {
    frame: &'a Frame,
    trade: Option<&'a TradeSection>,
}

impl<'a> HistoricalController<'a> {
    pub fn new(frame: &'a Frame, trade: Option<&'a TradeSection>) -> Self {
        HistoricalController { frame, trade }
    }

    pub async fn execute(&self) -> Result<DataFrame, String> {
        // Implement the logic to handle historical queries
        let df = match pull_data(&self.frame.model).await {
            Ok(data) => data,
            Err(e) => return Err(format!("Failed to pull data: {}", e)),
        };

        let mut action = action_over_data(&self.frame.actions, df);

        if self.trade.is_some() {
            let trade_section = self.trade.as_ref().unwrap();
            let df = trade_over_data(trade_section, action?);
            action = df;
        }

        action
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

pub fn trade_over_data(trade: &TradeSection, df: DataFrame) -> Result<DataFrame, String> {
    let n = df.height();
    let mut entry_col = vec![None; n];
    let mut exit_col = vec![None; n];
    let mut limit_col = vec![None; n];

    let close = match df
        .column("close")
        .map_err(|e| format!("Missing 'close' column: {}", e))?
        .f64()
    {
        Ok(col) => col.to_vec(),
        Err(e) => return Err(format!("Failed to get 'close' column: {}", e)),
    };

    let entry_fields: Vec<Vec<f64>> = match trade
        .entry
        .iter()
        .map(|f| {
            let col = df.column(f)?.f64()?.to_vec();
            Ok(col.into_iter().map(|v| v.unwrap_or(0.0)).collect())
        })
        .collect::<Result<_, PolarsError>>()
    {
        Ok(fields) => fields,
        Err(e) => return Err(format!("Failed to get entry fields: {}", e)),
    };

    let exit_fields: Vec<Vec<f64>> = match trade
        .exit
        .iter()
        .map(|f| {
            let col = df.column(f)?.f64()?.to_vec();
            Ok(col.into_iter().map(|v| v.unwrap_or(0.0)).collect())
        })
        .collect::<Result<_, PolarsError>>()
    {
        Ok(fields) => fields,
        Err(e) => return Err(format!("Failed to get exit fields: {}", e)),
    };

    let mut trade_id = 1;
    let mut i = 0;

    while i < n {
        let entry_score: f64 = entry_fields.iter().map(|col| col[i]).sum();

        if entry_score >= trade.within_entry {
            entry_col[i] = Some(trade_id);

            let entry_price = close[i];
            let mut j = i + 1;
            let mut exited = false;

            while j < n && (j - i) <= trade.hold as usize {
                let exit_score: f64 = exit_fields.iter().map(|col| col[j]).sum();
                let price = close[j];

                // stop loss condition
                let loss_triggered = match (entry_price, price, trade.trade_type.clone()) {
                    (Some(entry), Some(price), TradeType::OptionCall)
                    | (Some(entry), Some(price), TradeType::Stock) => {
                        price < entry - trade.stop_loss
                    }
                    (Some(entry), Some(price), TradeType::OptionPut) => {
                        price > entry + trade.stop_loss
                    }
                    _ => false, // If entry_price or price is None, do not trigger stop loss
                };

                if loss_triggered {
                    limit_col[j] = Some(trade_id);
                    exited = true;
                    break;
                }

                if exit_score >= trade.within_exit {
                    exit_col[j] = Some(trade_id);
                    exited = true;
                    break;
                }

                j += 1;
            }

            // force exit after hold
            if !exited && j < n {
                exit_col[j] = Some(trade_id);
            }

            trade_id += 1;
            i = j; // skip to after trade ends
        } else {
            i += 1;
        }
    }

    // Convert to Series
    let entry_series = Series::new("entry".into(), entry_col);
    let exit_series = Series::new("exit".into(), exit_col);
    let limit_series = Series::new("limit".into(), limit_col);

    let mut df = df.clone();
    df.with_column(entry_series).map_err(|e| e.to_string())?;
    df.with_column(exit_series).map_err(|e| e.to_string())?;
    df.with_column(limit_series).map_err(|e| e.to_string())?;

    Ok(df)
}

pub fn action_over_data(action: &ActionSection, df: DataFrame) -> Result<DataFrame, String> {
    let field = action.fields.clone();

    let timestamp = df
        .column("timestamp")
        .map_err(|e| format!("Failed to get timestamp column: {}", e))?
        .clone();

    let selected_fields = df
        .select(field.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .map_err(|e| format!("Failed to select fields: {}", e))?;

    let mut columns = vec![timestamp];
    columns.extend_from_slice(selected_fields.get_columns());

    // If calc is None, just return the timestamp + selected fields
    let Some(calcs) = &action.calc else {
        let result_df =
            DataFrame::new(columns).map_err(|e| format!("Failed to create DataFrame: {}", e))?;
        return Ok(result_df);
    };

    // Otherwise, run each Calc and append the results
    for calc in calcs {
        let calculation = Calculation::new(calc.clone());

        let calc_df = calculation.calculate(&df)?;
        columns.extend_from_slice(calc_df.get_columns());
    }

    let result_df =
        DataFrame::new(columns).map_err(|e| format!("Failed to create DataFrame: {}", e))?;
    Ok(result_df)
}
