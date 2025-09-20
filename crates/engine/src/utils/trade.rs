use crate::{parser::TradeSection, utils::trade};
use polars::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

/// TradeSection is a single function that each query can have
/// one of
///
/// ...
/// TRADE
///	    STOCK
///	    ENTRY aapl.low, nvda.sma, 0.05
///	    EXIT nvda.high, aapl.h_sma, 0.05
///	    LIMIT 0.1
///	    HOLD 14
///
/// trade over data will build a single DataFrame
/// shaped like this:
///     
/// /// | timestamp | entry | exit | limit |
/// /// | --------- | ----- | ---- | ----- |
///
/// timestamps will be from the dataframes used in the trade section
/// when the entry conditions are met
/// a uid will be generated for each trade
/// and the entry, exit and limit will be the uid
///
/// This function will process the trade_section and the frames to

/// Processes the `TradeSection` and the associated frames to generate a trade DataFrame
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TradeSummary {
    pub bar_chart_data: Vec<f64>,
    pub total_trades: usize,
    pub win_rate: f64,
    pub avg_win_per_1000: f64,
    pub avg_loss_per_1000: f64,
}

struct Trade {
    pub uid: String,

    pub entry_input: Option<Vec<Vec<Option<f64>>>>,
    pub exit_input: Option<Vec<Vec<Option<f64>>>>,

    pub entry_output: Vec<Option<String>>,
    pub exit_output: Vec<Option<String>>,
    pub limit_output: Vec<Option<String>>,
}

impl Trade {
    pub fn new(
        entry_input: Option<Vec<Vec<Option<f64>>>>,
        exit_input: Option<Vec<Vec<Option<f64>>>>,
    ) -> Self {
        Trade {
            uid: Uuid::new_v4().to_string(),
            entry_input,
            exit_input,
            entry_output: Vec::new(),
            exit_output: Vec::new(),
            limit_output: Vec::new(),
        }
    }

    pub fn new_uid(&mut self) {
        self.uid = Uuid::new_v4().to_string();
    }

    pub fn calculate(
        &mut self,
        entry_threshold: f64,
        exit_threshold: f64,
        limit: f64,
        hold: usize,
    ) -> Result<(), String> {
        {
            // make sure entry_input and exit_input are not None

            if self.entry_input.is_none() || self.exit_input.is_none() {
                return Err("Entry or exit input is None".to_string());
            }

            // For each entry_input
            // check that the difference between the values is within the entry_threshold
            // If so, add the uid to the entry_output
            // check that within the next `hold` rows, the exit_input is within the exit_threshold
            // If so, add the uid to the exit_output
            // If at any point during the hold period the limit threshold which is a percentage of the entry value is reached
            // add the uid to the limit_output
            // If the exit_input is not met within the hold period, the trade is considered closed
            // and the uid is added to the exit_output with a None value
            // If the exit_input is met, the uid is added to the exit_output with the
            // by default a None value will be added to the limit, entry and exit outputs

            let num_rows = if let Some(entry_input) = &self.entry_input {
                entry_input[0].len()
            } else {
                0
            };
            self.entry_output = vec![None; num_rows];
            self.exit_output = vec![None; num_rows];
            self.limit_output = vec![None; num_rows];

            if let (Some(entry_input), Some(exit_input)) = (&self.entry_input, &self.exit_input) {
                // entry_input and exit_input are Vec<Vec<Option<f64>>> where outer is columns, inner is rows
                // transpose to get Vec<Vec<Option<f64>>> where outer is rows, inner is columns
                let entry_rows: Vec<Vec<Option<f64>>> = (0..num_rows)
                    .map(|i| entry_input.iter().map(|col| col[i]).collect())
                    .collect();
                let exit_rows: Vec<Vec<Option<f64>>> = (0..num_rows)
                    .map(|i| exit_input.iter().map(|col| col[i]).collect())
                    .collect();

                let mut row = 0;
                while row < num_rows {
                    // Check entry condition
                    let entry = &entry_rows[row];
                    if entry.len() < 2 {
                        row += 1;
                        continue;
                    }
                    let mut is_valid_entry = true;
                    for i in 0..entry.len() - 1 {
                        if let (Some(a), Some(b)) = (entry[i], entry[i + 1]) {
                            if (a - b).abs() > entry_threshold {
                                is_valid_entry = false;
                                break;
                            }
                        } else {
                            is_valid_entry = false;
                            break;
                        }
                    }
                    if is_valid_entry {
                        let entry_uid = self.uid.clone();
                        self.entry_output[row] = Some(entry_uid.clone());
                        let entry_val = entry[0].unwrap_or(0.0);

                        // Look ahead for exit/limit within hold period
                        let mut closed = false;
                        for look_ahead in 1..=hold {
                            let idx = row + look_ahead;
                            if idx >= num_rows {
                                break;
                            }
                            let exit = &exit_rows[idx];
                            // Check exit condition
                            let mut is_valid_exit = true;
                            for i in 0..exit.len() - 1 {
                                if let (Some(a), Some(b)) = (exit[i], exit[i + 1]) {
                                    if (a - b).abs() > exit_threshold {
                                        is_valid_exit = false;
                                        break;
                                    }
                                } else {
                                    is_valid_exit = false;
                                    break;
                                }
                            }
                            if is_valid_exit {
                                self.exit_output[idx] = Some(entry_uid.clone());
                                closed = true;
                                self.new_uid();
                                break;
                            }
                            // Check limit (stop loss)
                            let limit_val = entry_val * (1.0 - limit);
                            if let Some(Some(val)) = exit.get(0) {
                                if *val < limit_val {
                                    self.limit_output[idx] = Some(entry_uid.clone());
                                    closed = true;
                                    self.new_uid();
                                    break;
                                }
                            }
                        }
                        if !closed {
                            // Trade closed after hold period, mark exit at last possible row
                            let close_idx = (row + hold).min(num_rows - 1);
                            self.exit_output[close_idx] = Some(entry_uid.clone());
                            self.new_uid();
                        }
                        row += hold; // skip to after hold period
                    } else {
                        row += 1;
                    }
                }
            }
        }

        Ok(())
    }
}

pub fn trades_over_data(
    trade_section: &TradeSection,
    frames: &HashMap<String, DataFrame>,
) -> Result<DataFrame, String> {
    // first we will create three vectors
    let mut timestamps: Vec<Option<i64>> = Vec::new();
    let mut entry: Vec<Vec<Option<f64>>> = Vec::new();
    let mut exit: Vec<Vec<Option<f64>>> = Vec::new();

    // second we will populate entry
    let entry_keys: Vec<(String, String)> = trade_section
        .entry
        .iter()
        .map(|str| {
            let split = str.split('.').collect::<Vec<&str>>();
            (split[0].to_string(), split[1].to_string())
        })
        .collect();

    match populate(entry_keys, frames) {
        Ok(data) => entry = data,
        Err(e) => return Err(format!("Failed to populate entry: {}", e)),
    };

    // third we will populate exit
    let exit_keys: Vec<(String, String)> = trade_section
        .exit
        .iter()
        .map(|str| {
            let split = str.split('.').collect::<Vec<&str>>();
            (split[0].to_string(), split[1].to_string())
        })
        .collect();

    match populate(exit_keys, frames) {
        Ok(data) => exit = data,
        Err(e) => return Err(format!("Failed to populate exit: {}", e)),
    };

    // fourth we will populate timestamps
    if let Some(first_frame) = frames.values().next() {
        timestamps = first_frame
            .column("timestamp")
            .map_err(|e| format!("Missing timestamp column: {}", e))?
            .i64()
            .map_err(|e| format!("Timestamp column not i64: {}", e))?
            .to_vec();
    } else {
        return Err("No frames provided".to_string());
    }

    // fifth we now implment Trade
    let mut trade = Trade::new(Some(entry), Some(exit));
    let entry_threshold = trade_section.within_entry;
    let exit_threshold = trade_section.within_exit;
    let limit = trade_section.stop_loss;
    let hold = trade_section.hold as usize;

    trade
        .calculate(entry_threshold, exit_threshold, limit, hold)
        .map_err(|e| format!("Trade calculation failed: {}", e))?;

    let df = match df!(
        "timestamp" => timestamps,
        "entry" => trade.entry_output,
        "exit" => trade.exit_output,
        "limit" => trade.limit_output,
    ) {
        Ok(df) => df,
        Err(e) => return Err(format!("Failed to create DataFrame: {}", e)),
    };

    let df = df
        .clone()
        .lazy()
        .filter(
            col("entry")
                .is_not_null()
                .or(col("exit").is_not_null())
                .or(col("limit").is_not_null()),
        )
        .collect()
        .unwrap_or(df.clone());

    // base lazy view from the filtered df you already built
    let base = df.clone().lazy();

    // 1) rows where entry fired -> (id, Entry)
    let entries = base
        .clone()
        .filter(col("entry").is_not_null())
        .select([col("entry").alias("id"), col("timestamp").alias("Entry")]);

    // 2) rows where exit fired -> (id, Exit)
    let exits = base
        .clone()
        .filter(col("exit").is_not_null())
        .select([col("exit").alias("id"), col("timestamp").alias("Exit")]);

    // 3) rows where stop/limit fired -> (id, Limit)
    let limits = base
        .filter(col("limit").is_not_null())
        .select([col("limit").alias("id"), col("timestamp").alias("Limit")]);

    // 4) left-join everything on id so Entry is required and Exit/Limit are optional
    let out = entries
        .left_join(exits, col("id"), col("id")) // <- no brackets
        .left_join(limits, col("id"), col("id")) // <- no brackets
        .filter(col("Exit").is_not_null().or(col("Limit").is_not_null()))
        .collect()
        .map_err(|e| format!("Failed to build trade summary: {}", e))?;

    // final shape:
    // | id   | Entry (i64 ts) | Exit (i64 ts, opt) | Limit (i64 ts, opt) |
    Ok(out)
}

// getter.0 is the the key of the frame in the hashmap
// getter.1 is the column name

fn populate(
    getter: Vec<(String, String)>,
    frames: &HashMap<String, DataFrame>,
) -> Result<Vec<Vec<Option<f64>>>, String> {
    // population grabs data from the frames based on the getter
    let mut data: Vec<Vec<Option<f64>>> = Vec::new();
    for (frame_key, column_name) in getter {
        if let Some(frame) = frames.get(&frame_key) {
            if let Ok(col) = frame.column(&column_name) {
                if let Ok(f64_col) = col.f64() {
                    data.push(f64_col.to_vec());
                } else {
                    return Err(format!(
                        "Column '{}' in frame '{}' is not f64",
                        column_name, frame_key
                    ));
                }
            } else {
                return Err(format!(
                    "Frame '{}' does not contain column '{}'",
                    frame_key, column_name
                ));
            }
        } else {
            return Err(format!(
                "Frame '{}' not found in provided frames",
                frame_key
            ));
        }
    }
    Ok(data)
}

pub fn trade_graphing_util(
    context: TradeSection,
    trades: &DataFrame,
    frame: &DataFrame,
) -> Vec<([[f64; 2]; 4], [[f64; 2]; 4])> {
    // objectively this will take the trades from from trades dataframe
    // then will map it over the frame dataframe
    // the output will be a vector of rectangles
    // the rectangles will be tuples of (x1, y1), (x2, y2), (x3, y3), (x4, y4)
    // the first rectangle in the tuple will be the buy and the second
    // rectangle will be the limit

    let mut rects: Vec<([[f64; 2]; 4], [[f64; 2]; 4])> = Vec::new();

    // Example: iterate over rows by index
    for idx in 0..trades.height() {
        // Access values by column and index, e.g.:
        // let entry = trades.column("Entry").unwrap().i64().unwrap().get(idx);
        // let exit = trades.column("Exit").unwrap().i64().unwrap().get(idx);
        // ... build your rectangle here ...

        let left_x = trades
            .column("Entry")
            .unwrap()
            .i64()
            .unwrap()
            .get(idx)
            .unwrap_or(0) as f64;

        // Use Exit if present, otherwise use Limit
        let right_x = trades
            .column("Exit")
            .unwrap()
            .i64()
            .unwrap()
            .get(idx)
            .or_else(|| trades.column("Limit").unwrap().i64().unwrap().get(idx))
            .unwrap_or(left_x as i64 + 1) as f64;

        let limit_buy_intercept = frame
            .clone()
            .lazy()
            .filter(col("timestamp").eq(lit(left_x as i64)))
            .select([col("open")])
            .collect()
            .ok()
            .and_then(|df| {
                if df.height() > 0 {
                    df.column("open").ok()?.f64().ok()?.get(0)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);

        let limit_down = limit_buy_intercept * (1.0 - context.stop_loss);

        let buy_up = frame
            .clone()
            .lazy()
            .filter(col("timestamp").eq(lit(right_x as i64)))
            .select([col("open")])
            .collect()
            .ok()
            .and_then(|df| {
                if df.height() > 0 {
                    df.column("open").ok()?.f64().ok()?.get(0)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);

        let buy_rect = [
            [left_x, buy_up],
            [right_x, buy_up],
            [right_x, limit_buy_intercept],
            [left_x, limit_buy_intercept],
        ];

        let limit_rect = [
            [left_x, limit_buy_intercept],
            [right_x, limit_buy_intercept],
            [right_x, limit_down],
            [left_x, limit_down],
        ];

        rects.push((buy_rect, limit_rect));
    }

    rects
}

pub fn trade_summary_util(
    context: TradeSection,
    trades: &DataFrame,
    frame: &DataFrame,
) -> TradeSummary {
    let mut trade_summary = TradeSummary::default();

    for idx in 0..trades.height() {
        let left_x = trades
            .column("Entry")
            .unwrap()
            .i64()
            .unwrap()
            .get(idx)
            .unwrap_or(0) as f64;

        // Use Exit if present, otherwise use Limit
        let limit: Option<f64> = trades
            .column("Limit")
            .unwrap()
            .i64()
            .unwrap()
            .get(idx)
            .and_then(|exit_ts| {
                frame
                    .clone()
                    .lazy()
                    .filter(col("timestamp").eq(lit(exit_ts)))
                    .select([col("open")])
                    .collect()
                    .ok()
                    .and_then(|df| {
                        if df.height() > 0 {
                            df.column("open").ok()?.f64().ok()?.get(0)
                        } else {
                            None
                        }
                    })
            });

        let profit: Option<f64> = trades
            .column("Exit")
            .unwrap()
            .i64()
            .unwrap()
            .get(idx)
            .and_then(|exit_ts| {
                frame
                    .clone()
                    .lazy()
                    .filter(col("timestamp").eq(lit(exit_ts)))
                    .select([col("open")])
                    .collect()
                    .ok()
                    .and_then(|df| {
                        if df.height() > 0 {
                            df.column("open").ok()?.f64().ok()?.get(0)
                        } else {
                            None
                        }
                    })
            });

        let entry = frame
            .clone()
            .lazy()
            .filter(col("timestamp").eq(lit(left_x as i64)))
            .select([col("open")])
            .collect()
            .ok()
            .and_then(|df| {
                if df.height() > 0 {
                    df.column("open").ok()?.f64().ok()?.get(0)
                } else {
                    None
                }
            })
            .unwrap_or(0.0);

        if let Some(limit) = limit {
            trade_summary.bar_chart_data.push(limit - entry);

            trade_summary.avg_loss_per_1000 += entry / 1000.0 * (limit - entry);
        }

        if let Some(profit) = profit {
            trade_summary.bar_chart_data.push(profit - entry);

            trade_summary.avg_win_per_1000 += entry / 1000.0 * (profit - entry);
        }
    }

    trade_summary.total_trades = trade_summary.bar_chart_data.len();
    trade_summary.avg_loss_per_1000 /= trade_summary.total_trades as f64;
    trade_summary.avg_win_per_1000 /= trade_summary.total_trades as f64;
    // Win rate is the ratio of positive trades to negative trades (ignoring zeros)
    let positives = trade_summary
        .bar_chart_data
        .iter()
        .filter(|&&x| x > 0.0)
        .count();
    let negatives = trade_summary
        .bar_chart_data
        .iter()
        .filter(|&&x| x < 0.0)
        .count();
    let denom = positives + negatives;
    trade_summary.win_rate = if denom > 0 {
        (positives as f64 / denom as f64) * 100.0
    } else {
        0.0
    };

    trade_summary
}
