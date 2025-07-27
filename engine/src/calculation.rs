use crate::{lexer::Keyword, parser::Calc};
use polars::prelude::*;

pub struct Calculation(Calc);

impl Calculation {
    pub fn new(calc: Calc) -> Self {
        Calculation(calc)
    }

    pub fn calculate(&self, df: &DataFrame) -> Result<DataFrame, String> {
        let data = match df.columns(&self.0.inputs) {
            Ok(data) => data,
            Err(e) => return Err(format!("Failed to get columns: {}", e)),
        };

        let data: Vec<&Series> = data.iter().map(|s| s.as_materialized_series()).collect();

        let data: Vec<Vec<Option<f64>>> = data
            .iter()
            .map(|s| s.f64().unwrap().into_iter().collect())
            .collect();

        match self.0.operation {
            Keyword::Difference => {
                let mut diffs = Vec::new();
                for row in 0..data[0].len() {
                    let mut row_vals = Vec::new();
                    for col in &data {
                        row_vals.push(col[row]);
                    }
                    // Compute difference between each consecutive value in the row
                    let mut row_diffs = Vec::new();
                    for i in 1..row_vals.len() {
                        match (row_vals[i], row_vals[i - 1]) {
                            (Some(curr), Some(prev)) => row_diffs.push(Some(curr - prev)),
                            _ => row_diffs.push(None),
                        }
                    }
                    diffs.push(row_diffs);
                }

                // Convert diffs to Series and then to DataFrame
                let mut series_vec = Vec::new();
                for i in 0..(data.len() - 1) {
                    let col_name = self.0.alias.clone();
                    let col_data: Vec<Option<f64>> = diffs.iter().map(|row| row[i]).collect();
                    series_vec.push(Series::new(col_name.into(), col_data));
                }
                let columns: Vec<Column> =
                    series_vec.into_iter().map(|s| s.into_column()).collect();
                DataFrame::new(columns).map_err(|e| format!("Failed to create DataFrame: {}", e))
            }
            Keyword::Sma => {
                let period = 14; // Default to 14 if not specified
                let mut sma_values = Vec::new();
                for i in 0..data[0].len() {
                    if i + 1 < period {
                        sma_values.push(None); // Not enough data for SMA
                    } else {
                        let start = i + 1 - period;
                        let sum: f64 = data[0][start..=i].iter().filter_map(|&x| x).sum();
                        let sma = sum / period as f64;
                        sma_values.push(Some(sma));
                    }
                }
                // Shift the first [0..period] elements to the end of the array
                let mut shifted_sma_values = sma_values;
                let front = shifted_sma_values.drain(0..period/2).collect::<Vec<_>>();
                shifted_sma_values.extend(front);

                let sma_values = shifted_sma_values
                    .into_iter()
                    .map(|x| x.unwrap_or(0.0)) // Replace None with 0.0 for consistency
                    .collect::<Vec<f64>>();
                let name = self.0.alias.clone();
                let series = Series::new(name.into(), sma_values);
                DataFrame::new(vec![series.into_column()])
                    .map_err(|e| format!("Failed to create DataFrame: {}", e))
            }
            Keyword::Volatility => {
                let period = 14; // Default to 14 if not specified
                let mut volatility_values = Vec::new();
                for i in 0..data[0].len() {
                    if i < period - 1 {
                        volatility_values.push(None); // Not enough data for volatility
                    } else {
                        let mean: f64 = data[0][i - period + 1..=i]
                            .iter()
                            .filter_map(|&x| x)
                            .sum::<f64>()
                            / period as f64;

                        let variance: f64 = data[0][i - period + 1..=i]
                            .iter()
                            .filter_map(|&x| x)
                            .map(|x| (x - mean).powi(2))
                            .sum::<f64>()
                            / period as f64;

                        let volatility = variance.sqrt();
                        volatility_values.push(Some(volatility));
                    }
                }
                let series = Series::new(self.0.alias.clone().into(), volatility_values);
                DataFrame::new(vec![series.into_column()])
                    .map_err(|e| format!("Failed to create DataFrame: {}", e))
            }
            _ => Err(format!("Unsupported operation: {:?}", self.0.operation)),
        }
    }
}
