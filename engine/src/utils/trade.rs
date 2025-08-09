use crate::parser::{TradeSection, TradeType};
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

    // for debug
    println!("Available frames: {:?}", frames.keys().collect::<Vec<_>>());

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

    match df!(
        "timestamp" => timestamps,
        "entry" => trade.entry_output,
        "exit" => trade.exit_output,
        "limit" => trade.limit_output,
    ) {
        Ok(df) => Ok(df),
        Err(e) => Err(format!("Failed to create DataFrame: {}", e)),
    }
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

pub fn trade_dataframe_to_table(
    trades: &DataFrame,
    dataset: &DataFrame,
) -> Result<DataFrame, String> {
    // trades is going to have uids assigned to trades
    // we will use these uids to join with the dataset
    // dataset is the original data that was used to generate the trades
    // then the function will return a DataFrame with columns :
    // uid entry_price, entry_timestamp, exit_price, exit_timestamp, limit_price, limit_timestamp
    // these can have None values, the datum is the entry every trade starts with an entry
    // and then it can have an exit or a limit
    Err("Not implemented".to_string())
}
