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
                let front = shifted_sma_values.drain(0..period / 2).collect::<Vec<_>>();
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
                // `data[0]` is assumed to be Vec<Option<f64>> of closing prices.
                let period: usize = 14;

                // 1) Log returns
                let mut log_ret: Vec<Option<f64>> = Vec::with_capacity(data[0].len());
                log_ret.push(None); // no return at t=0
                for i in 1..data[0].len() {
                    match (data[0][i - 1], data[0][i]) {
                        (Some(p0), Some(p1)) if p0 > 0.0 && p1 > 0.0 => {
                            log_ret.push(Some((p1 / p0).ln()))
                        }
                        _ => log_ret.push(None),
                    }
                }

                // 2) Rolling stddev of returns (sample std with n-1; change to `period` for population)
                let mut vol: Vec<Option<f64>> = Vec::with_capacity(log_ret.len());
                for i in 0..log_ret.len() {
                    if i + 1 < period {
                        vol.push(None);
                        continue;
                    }
                    let start = i + 1 - period;
                    let window: Vec<f64> = log_ret[start..=i].iter().filter_map(|&x| x).collect();
                    if window.len() < period {
                        vol.push(None);
                        continue;
                    }
                    let mean = window.iter().sum::<f64>() / window.len() as f64;
                    let var = window.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                        / ((window.len() - 1) as f64);
                    let std = var.sqrt();

                    // Optional: annualize (daily data -> * sqrt(252))
                    let annualized = std * 252f64.sqrt();

                    vol.push(Some(annualized)); // or push `std` if you prefer non-annualized
                }

                // 3) Keep nulls; don't zero-fill
                let name = format!("{}", self.0.alias);
                let series = Series::new(name.clone().into(), vol.clone()); // Vec<Option<f64>> is supported

                // 4) create two series formatted as format!("{}-pos", name)
                //.   format!("{}-neg", name)
                // these values will be the selected series * (1 + val) in the positive and
                // series * (1 - val) in the negative

                let pos_series = Series::new(
                    format!("{}_pos", name.clone()).into(),
                    vol.iter()
                        .enumerate()
                        .map(|(i, v)| {
                            if v.is_none() {
                                None
                            } else {
                                match data[0][i] {
                                    Some(price) => Some(price * (1.0 + (v.unwrap() / 2.0))),
                                    None => None,
                                }
                            }
                        })
                        .collect::<Vec<_>>(),
                );
                let neg_series = Series::new(
                    format!("{}_neg", name.clone()).into(),
                    vol.iter()
                        .enumerate()
                        .map(|(i, v)| {
                            if v.is_none() {
                                None
                            } else {
                                match data[0][i] {
                                    Some(price) => Some(price * (1.0 - (v.unwrap() / 2.0))),
                                    None => None,
                                }
                            }
                        })
                        .collect::<Vec<_>>(),
                );

                DataFrame::new(vec![
                    series.into_column(),
                    pos_series.into_column(),
                    neg_series.into_column(),
                ])
                .map_err(|e| format!("Failed to create DataFrame: {}", e))
            }

            Keyword::DoubleVolatility => {
                // Compute double volatility (2 * volatility)

                // `data[0]` is assumed to be Vec<Option<f64>> of closing prices.
                let period: usize = 14;

                // 1) Log returns
                let mut log_ret: Vec<Option<f64>> = Vec::with_capacity(data[0].len());
                log_ret.push(None); // no return at t=0
                for i in 1..data[0].len() {
                    match (data[0][i - 1], data[0][i]) {
                        (Some(p0), Some(p1)) if p0 > 0.0 && p1 > 0.0 => {
                            log_ret.push(Some((p1 / p0).ln()))
                        }
                        _ => log_ret.push(None),
                    }
                }

                // 2) Rolling stddev of returns (sample std with n-1; change to `period` for population)
                let mut vol: Vec<Option<f64>> = Vec::with_capacity(log_ret.len());
                for i in 0..log_ret.len() {
                    if i + 1 < period {
                        vol.push(None);
                        continue;
                    }
                    let start = i + 1 - period;
                    let window: Vec<f64> = log_ret[start..=i].iter().filter_map(|&x| x).collect();
                    if window.len() < period {
                        vol.push(None);
                        continue;
                    }
                    let mean = window.iter().sum::<f64>() / window.len() as f64;
                    let var = window.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                        / ((window.len() - 1) as f64);
                    let std = var.sqrt();

                    // Optional: annualize (daily data -> * sqrt(252))
                    let annualized = std * 252f64.sqrt();

                    vol.push(Some(annualized)); // or push `std` if you prefer non-annualized
                }

                // 3) Keep nulls; don't zero-fill
                let name = format!("{}", self.0.alias);
                let series = Series::new(name.clone().into(), vol.clone()); // Vec<Option<f64>> is supported

                // 4) create two series formatted as format!("{}-pos", name)
                //.   format!("{}-neg", name)
                // these values will be the selected series * (1 + val) in the positive and
                // series * (1 - val) in the negative

                let pos_series = Series::new(
                    format!("{}_pos", name.clone()).into(),
                    vol.iter()
                        .enumerate()
                        .map(|(i, v)| {
                            if v.is_none() {
                                None
                            } else {
                                match data[0][i] {
                                    Some(price) => Some(price * (1.0 + (v.unwrap() / 1.0))),
                                    None => None,
                                }
                            }
                        })
                        .collect::<Vec<_>>(),
                );
                let neg_series = Series::new(
                    format!("{}_neg", name.clone()).into(),
                    vol.iter()
                        .enumerate()
                        .map(|(i, v)| {
                            if v.is_none() {
                                None
                            } else {
                                match data[0][i] {
                                    Some(price) => Some(price * (1.0 - (v.unwrap() / 1.0))),
                                    None => None,
                                }
                            }
                        })
                        .collect::<Vec<_>>(),
                );

                DataFrame::new(vec![
                    series.into_column(),
                    pos_series.into_column(),
                    neg_series.into_column(),
                ])
                .map_err(|e| format!("Failed to create DataFrame: {}", e))
            }

            Keyword::LinearRegression => {
                /// Fit simple linear regression y = a*x + b on (x,y) with x = 0..n-1 (only using valid y’s).
                fn fit_ols_indexed(y_opt: &[Option<f64>]) -> Option<(f64, f64)> {
                    // Collect valid (x, y)
                    let mut xs = Vec::new();
                    let mut ys = Vec::new();
                    for (i, y) in y_opt.iter().enumerate() {
                        if let Some(v) = y {
                            xs.push(i as f64);
                            ys.push(*v);
                        }
                    }
                    if xs.len() < 2 {
                        return None; // not enough data
                    }

                    // Mean-center for numerical stability
                    let n = xs.len() as f64;
                    let mean_x = xs.iter().sum::<f64>() / n;
                    let mean_y = ys.iter().sum::<f64>() / n;

                    let mut sxx = 0.0;
                    let mut sxy = 0.0;
                    for (x, y) in xs.iter().zip(ys.iter()) {
                        let dx = x - mean_x;
                        sxx += dx * dx;
                        sxy += dx * (y - mean_y);
                    }
                    if sxx == 0.0 {
                        return None; // all x equal (shouldn't happen with indices, but guard anyway)
                    }
                    let slope = sxy / sxx;
                    let intercept = mean_y - slope * mean_x;
                    Some((slope, intercept))
                }

                /// Build fitted line series (either absolute ŷ or relative to first valid open).
                fn fitted_line_series(
                    alias: &str,
                    y_opt: &[Option<f64>],
                    relative_to_first_open: bool,
                ) -> Series {
                    let (slope, intercept) = match fit_ols_indexed(y_opt) {
                        Some(si) => si,
                        None => (0.0, y_opt.iter().flatten().copied().next().unwrap_or(0.0)), // flat line fall-back
                    };

                    // Absolute fitted line: ŷ[i] = a*i + b
                    let y_hat: Vec<f64> = (0..y_opt.len())
                        .map(|i| slope * (i as f64) + intercept)
                        .collect();

                    if relative_to_first_open {
                        // Normalize to first valid open: return (ŷ / y0 - 1.0)
                        if let Some(y0) = y_opt.iter().flatten().copied().next() {
                            let rel: Vec<f64> = y_hat.iter().map(|v| v / y0 - 1.0).collect();
                            Series::new(alias.into(), rel)
                        } else {
                            Series::new(alias.into(), y_hat) // nothing to normalize against
                        }
                    } else {
                        Series::new(alias.into(), y_hat)
                    }
                }
                // Build Option<f64> vector from your data (don’t coerce missing to 0.0 for the fit)
                let y_opt: Vec<Option<f64>> = data[0].clone(); // if you already have Vec<Option<f64>>

                // Create the fitted line series (set the flag depending on what you want)
                let series = fitted_line_series(
                    &self.0.alias,
                    &y_opt,
                    /* relative_to_first_open = */ false,
                );

                // If you actually want it relative to the first open, set true.

                DataFrame::new(vec![series.into_column()])
                    .map_err(|e| format!("Failed to create DataFrame: {}", e))
            }

            _ => Err(format!("Unsupported operation: {:?}", self.0.operation)),
        }
    }
}
