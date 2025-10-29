use crate::parser::TradeSection;
use polars::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TradeSummary {
    pub bar_chart_data: Vec<f64>,
    pub total_trades: usize,
    pub win_rate: f64,
    pub avg_win_per_1000: f64,
    pub avg_loss_per_1000: f64,
}

type Matrix = Vec<Vec<Option<f64>>>;

struct Trade {
    uid: String,
    entry_cols: Matrix,
    exit_cols: Matrix,
    entry_out: Vec<Option<String>>,
    exit_out: Vec<Option<String>>,
    limit_out: Vec<Option<String>>,
}

impl Trade {
    fn new(entry_cols: Matrix, exit_cols: Matrix) -> Result<Self, String> {
        let rows = Self::ensure_same_height(&entry_cols, &exit_cols)?;
        Ok(Self {
            uid: Uuid::new_v4().to_string(),
            entry_cols,
            exit_cols,
            entry_out: vec![None; rows],
            exit_out: vec![None; rows],
            limit_out: vec![None; rows],
        })
    }
    #[inline]
    fn reset_uid(&mut self) {
        self.uid = Uuid::new_v4().to_string();
    }
    #[inline]
    fn height(&self) -> usize {
        self.entry_cols.get(0).map(|c| c.len()).unwrap_or(0)
    }
    #[inline]
    fn width_entry(&self) -> usize {
        self.entry_cols.len()
    }
    #[inline]
    fn width_exit(&self) -> usize {
        self.exit_cols.len()
    }

    fn ensure_same_height(a: &Matrix, b: &Matrix) -> Result<usize, String> {
        let ah = a.get(0).map(|c| c.len()).unwrap_or(0);
        let bh = b.get(0).map(|c| c.len()).unwrap_or(0);
        if !a.iter().all(|c| c.len() == ah) {
            return Err("entry columns have mismatched lengths".into());
        }
        if !b.iter().all(|c| c.len() == bh) {
            return Err("exit columns have mismatched lengths".into());
        }
        if ah != bh {
            return Err(format!(
                "entry/exit heights differ (entry: {ah}, exit: {bh})"
            ));
        }
        Ok(ah)
    }

    #[inline]
    fn row_within(cols: &Matrix, row: usize, thr: f64) -> bool {
        let w = cols.len();
        if w < 2 {
            return false;
        }
        for i in 0..(w - 1) {
            match (cols[i][row], cols[i + 1][row]) {
                (Some(a), Some(b)) if (a - b).abs() <= thr => {}
                _ => return false,
            }
        }
        true
    }

    fn calculate(
        &mut self,
        entry_thr: f64,
        exit_thr: f64,
        stop_loss: f64,
        hold: usize,
    ) -> Result<(), String> {
        let n = self.height();
        if n == 0 || self.width_entry() < 2 || self.width_exit() < 1 {
            return Ok(());
        }

        let mut row = 0;
        while row < n {
            if !Self::row_within(&self.entry_cols, row, entry_thr) {
                row += 1;
                continue;
            }

            let entry_uid = self.uid.clone();
            self.entry_out[row] = Some(entry_uid.clone());
            let entry_val = self.entry_cols[0][row].unwrap_or(0.0);
            let limit_val = entry_val * (1.0 - stop_loss);

            let mut closed = false;
            for la in 1..=hold {
                let idx = row + la;
                if idx >= n {
                    break;
                }

                if Self::row_within(&self.exit_cols, idx, exit_thr) {
                    self.exit_out[idx] = Some(entry_uid.clone());
                    closed = true;
                    self.reset_uid();
                    break;
                }

                if let Some(Some(v)) = self.exit_cols.get(0).map(|c| c[idx]) {
                    if v < limit_val {
                        self.limit_out[idx] = Some(entry_uid.clone());
                        closed = true;
                        self.reset_uid();
                        break;
                    }
                }
            }

            if !closed {
                let close_idx = (row + hold).min(n - 1);
                self.exit_out[close_idx] = Some(entry_uid.clone());
                self.reset_uid();
            }

            row += hold.max(1);
        }

        Ok(())
    }
}

fn series_as_f64_opt(col: &Series) -> Result<Vec<Option<f64>>, String> {
    use polars::prelude::DataType::*;
    match col.dtype() {
        Float64 => Ok(col
            .f64()
            .map_err(|e| format!("to f64 failed: {e}"))?
            .to_vec()),
        Float32 => Ok(col
            .f32()
            .map_err(|e| format!("to f32 failed: {e}"))?
            .into_iter()
            .map(|o| o.map(|v| v as f64))
            .collect()),
        Int64 => Ok(col
            .i64()
            .map_err(|e| format!("to i64 failed: {e}"))?
            .into_iter()
            .map(|o| o.map(|v| v as f64))
            .collect()),
        Int32 => Ok(col
            .i32()
            .map_err(|e| format!("to i32 failed: {e}"))?
            .into_iter()
            .map(|o| o.map(|v| v as f64))
            .collect()),
        UInt64 => Ok(col
            .u64()
            .map_err(|e| format!("to u64 failed: {e}"))?
            .into_iter()
            .map(|o| o.map(|v| v as f64))
            .collect()),
        UInt32 => Ok(col
            .u32()
            .map_err(|e| format!("to u32 failed: {e}"))?
            .into_iter()
            .map(|o| o.map(|v| v as f64))
            .collect()),
        other => Err(format!("unsupported dtype for trades: {other:?}")),
    }
}

fn populate(
    getter: Vec<(String, String)>,
    frames: &HashMap<String, DataFrame>,
) -> Result<Matrix, String> {
    let mut data: Matrix = Vec::with_capacity(getter.len());
    for (frame_key, column_name) in getter {
        let frame = frames
            .get(&frame_key)
            .ok_or_else(|| format!("frame '{frame_key}' not found"))?;
        let col = frame
            .column(&column_name)
            .map_err(|_| format!("frame '{frame_key}' missing column '{column_name}'"))?;
        let vec = series_as_f64_opt(col.as_series().unwrap())?;
        data.push(vec);
    }
    Ok(data)
}

fn build_open_lookup(frame: &DataFrame) -> Result<HashMap<i64, f64>, String> {
    let ts = frame
        .column("timestamp")
        .map_err(|e| format!("frame missing 'timestamp': {e}"))?
        .i64()
        .map_err(|e| format!("'timestamp' not i64: {e}"))?;
    let open = frame
        .column("open")
        .map_err(|e| format!("frame missing 'open': {e}"))?
        .f64()
        .map_err(|e| format!("'open' not f64: {e}"))?;
    if ts.len() != open.len() {
        return Err("timestamp/open length mismatch".into());
    }
    let mut map = HashMap::with_capacity(ts.len());
    for i in 0..ts.len() {
        if let (Some(t), Some(o)) = (ts.get(i), open.get(i)) {
            map.insert(t, o);
        }
    }
    Ok(map)
}

pub fn trade_graphing_util(
    context: TradeSection,
    trades: &DataFrame,
    frame: &DataFrame,
) -> Vec<([[f64; 2]; 4], [[f64; 2]; 4])> {
    let mut rects = Vec::with_capacity(trades.height().max(1));
    let lookup = match build_open_lookup(frame) {
        Ok(m) => m,
        Err(_) => return rects,
    };

    let entry_ca = trades.column("Entry").ok().and_then(|s| s.i64().ok());
    let exit_ca = trades.column("Exit").ok().and_then(|s| s.i64().ok());
    let limit_ca = trades.column("Limit").ok().and_then(|s| s.i64().ok());

    for idx in 0..trades.height() {
        let left_ts = entry_ca.and_then(|c| c.get(idx)).unwrap_or(0);
        let right_ts = exit_ca
            .and_then(|c| c.get(idx))
            .or_else(|| limit_ca.and_then(|c| c.get(idx)))
            .unwrap_or(left_ts + 1);

        let entry_open = lookup.get(&left_ts).copied().unwrap_or(0.0);
        let right_open = lookup.get(&right_ts).copied().unwrap_or(0.0);
        let limit_down = entry_open * (1.0 - context.stop_loss);

        let buy_rect = [
            [left_ts as f64, right_open],
            [right_ts as f64, right_open],
            [right_ts as f64, entry_open],
            [left_ts as f64, entry_open],
        ];
        let limit_rect = [
            [left_ts as f64, entry_open],
            [right_ts as f64, entry_open],
            [right_ts as f64, limit_down],
            [left_ts as f64, limit_down],
        ];
        rects.push((buy_rect, limit_rect));
    }
    rects
}

pub fn trade_summary_util(
    _context: TradeSection,
    trades: &DataFrame,
    frame: &DataFrame,
) -> TradeSummary {
    let mut tsum = TradeSummary::default();
    let lookup = match build_open_lookup(frame) {
        Ok(m) => m,
        Err(_) => return tsum,
    };

    let entry_ca = match trades.column("Entry").and_then(|s| s.i64()) {
        Ok(ca) => ca,
        Err(_) => return tsum,
    };
    let exit_ca = trades.column("Exit").ok().and_then(|s| s.i64().ok());
    let limit_ca = trades.column("Limit").ok().and_then(|s| s.i64().ok());

    for idx in 0..trades.height() {
        let entry_ts = entry_ca.get(idx).unwrap_or(0);
        let entry = lookup.get(&entry_ts).copied().unwrap_or(0.0);

        if let Some(limit_ts) = limit_ca.and_then(|c| c.get(idx)) {
            if let Some(limit) = lookup.get(&limit_ts) {
                tsum.bar_chart_data.push(limit - entry);
                tsum.avg_loss_per_1000 += entry / 1000.0 * (limit - entry);
            }
        }
        if let Some(exit_ts) = exit_ca.and_then(|c| c.get(idx)) {
            if let Some(profit) = lookup.get(&exit_ts) {
                tsum.bar_chart_data.push(profit - entry);
                tsum.avg_win_per_1000 += entry / 1000.0 * (profit - entry);
            }
        }
    }

    tsum.total_trades = tsum.bar_chart_data.len();
    if tsum.total_trades > 0 {
        tsum.avg_loss_per_1000 /= tsum.total_trades as f64;
        tsum.avg_win_per_1000 /= tsum.total_trades as f64;
        let positives = tsum.bar_chart_data.iter().filter(|&&x| x > 0.0).count();
        let negatives = tsum.bar_chart_data.iter().filter(|&&x| x < 0.0).count();
        let denom = positives + negatives;
        tsum.win_rate = if denom > 0 {
            (positives as f64 / denom as f64) * 100.0
        } else {
            0.0
        };
    }
    tsum
}

//* -------- parse "frame.col" -------- */
fn split_key_col(s: &String) -> Result<(String, String), String> {
    let mut it = s.splitn(2, '.');
    let f = it.next().ok_or("missing frame key")?;
    let c = it.next().ok_or("missing column name")?;
    Ok((f.to_string(), c.to_string()))
}

/* -------- two-column (timestamp, value) slice -------- */
fn slice_frame(df: &DataFrame, value_col: &str, alias: &str) -> Result<DataFrame, String> {
    // timestamp as Series<i64>
    let ts: Series = df
        .column("timestamp")
        .map_err(|e| format!("frame missing 'timestamp': {e}"))?
        .i64()
        .map_err(|e| format!("'timestamp' not i64: {e}"))?
        .clone()
        .into_series(); // name should already be "timestamp"

    let val: Series = col_as_f64_series(df, value_col, alias)?;
    DataFrame::new(vec![ts.into_column(), val.into_column()]).map_err(|e| e.to_string())
}

/* -------- inner-join on timestamp (Polars 0.49 signature) -------- */
fn inner_join_on_timestamp(acc: DataFrame, next: DataFrame) -> Result<DataFrame, String> {
    acc.join(
        &next,
        ["timestamp"],
        ["timestamp"],
        JoinArgs::new(JoinType::Inner),
        None, // <— extra arg in 0.49: Option<JoinTypeOptions>
    )
    .map_err(|e| format!("join on timestamp failed: {e}"))
}

/* -------- build one aligned table of all referenced series -------- */
fn align_all_by_timestamp(
    entry_keys: &[(String, String)],
    exit_keys: &[(String, String)],
    frames: &HashMap<String, DataFrame>,
) -> Result<(DataFrame, Vec<String>, Vec<String>), String> {
    let mut tables: Vec<DataFrame> = Vec::new();
    let mut entry_aliases: Vec<String> = Vec::new();
    let mut exit_aliases: Vec<String> = Vec::new();

    for (i, (fk, ck)) in entry_keys.iter().enumerate() {
        let alias = format!("E{i}");
        let f = frames
            .get(fk)
            .ok_or_else(|| format!("frame '{fk}' not found"))?;
        tables.push(slice_frame(f, ck, &alias)?);
        entry_aliases.push(alias);
    }
    for (i, (fk, ck)) in exit_keys.iter().enumerate() {
        let alias = format!("X{i}");
        let f = frames
            .get(fk)
            .ok_or_else(|| format!("frame '{fk}' not found"))?;
        tables.push(slice_frame(f, ck, &alias)?);
        exit_aliases.push(alias);
    }

    let mut it = tables.into_iter();
    let mut aligned = it
        .next()
        .ok_or_else(|| "no columns requested for trades".to_string())?;
    for t in it {
        aligned = inner_join_on_timestamp(aligned, t)?;
    }

    aligned = aligned
        .lazy()
        .sort(["timestamp"], SortMultipleOptions::default())
        .collect()
        .map_err(|e| format!("sort failed: {e}"))?;

    Ok((aligned, entry_aliases, exit_aliases))
}

/* -------- matrix extraction -------- */
fn to_matrix_opt_f64(df: &DataFrame, cols: &[String]) -> Result<Vec<Vec<Option<f64>>>, String> {
    let mut m: Vec<Vec<Option<f64>>> = Vec::with_capacity(cols.len());
    for name in cols {
        let ca = df
            .column(name)
            .map_err(|_| format!("aligned df missing col '{name}'"))?
            .f64()
            .map_err(|e| format!("'{name}' not f64 after alignment: {e}"))?;
        m.push(ca.to_vec());
    }
    Ok(m)
}

pub fn trades_over_data(
    trade_section: &TradeSection,
    frames: &HashMap<String, DataFrame>,
) -> Result<DataFrame, String> {
    let entry_keys = trade_section
        .entry
        .iter()
        .map(split_key_col)
        .collect::<Result<Vec<_>, _>>()?;
    let exit_keys = trade_section
        .exit
        .iter()
        .map(split_key_col)
        .collect::<Result<Vec<_>, _>>()?;

    let entry_cols = populate(entry_keys, frames)?;
    let exit_cols = populate(exit_keys, frames)?;

    // Build timestamps from the first available frame (kept for the intermediate filter step)
    let timestamps: Vec<Option<i64>> = match frames.values().next() {
        Some(df0) => df0
            .column("timestamp")
            .map_err(|e| format!("Missing timestamp: {e}"))?
            .i64()
            .map_err(|e| format!("Timestamp not i64: {e}"))?
            .to_vec(),
        None => return Ok(empty_trades_output()), // no frames at all
    };

    // Construct the working Trade; this is where your earlier error came from.
    let mut trade = match Trade::new(entry_cols, exit_cols) {
        Ok(t) => t,
        Err(e) => {
            // Gracefully handle the “no overlapping timestamps” situation:
            if e.contains("entry/exit heights differ") || e.contains("no overlapping timestamps") {
                return Ok(empty_trades_output());
            }
            return Err(e);
        }
    };

    // If there are zero rows, nothing to do—return empty output with expected schema.
    if trade.height() == 0 {
        return Ok(empty_trades_output());
    }

    trade.calculate(
        trade_section.within_entry,
        trade_section.within_exit,
        trade_section.stop_loss,
        trade_section.hold as usize,
    )?;

    // Build the intermediate (timestamp, entry/exit/limit flags) df
    let df = df![
        "timestamp" => timestamps,
        "entry" => trade.entry_out,
        "exit"  => trade.exit_out,
        "limit" => trade.limit_out,
    ]
    .map_err(|e| format!("Failed to create DataFrame: {e}"))?;

    // Keep only rows that have any marker
    let filtered = df
        .clone()
        .lazy()
        .filter(
            col("entry")
                .is_not_null()
                .or(col("exit").is_not_null())
                .or(col("limit").is_not_null()),
        )
        .collect()
        .unwrap_or(df);

    if filtered.height() == 0 {
        return Ok(empty_trades_output());
    }

    // Build the final (id, Entry, Exit, Limit) table expected by your downstream utilities
    let base = filtered.lazy();

    let entries = base
        .clone()
        .filter(col("entry").is_not_null())
        .select([col("entry").alias("id"), col("timestamp").alias("Entry")]);

    let exits = base
        .clone()
        .filter(col("exit").is_not_null())
        .select([col("exit").alias("id"), col("timestamp").alias("Exit")]);

    let limits = base
        .filter(col("limit").is_not_null())
        .select([col("limit").alias("id"), col("timestamp").alias("Limit")]);

    let out = entries
        .left_join(exits, col("id"), col("id"))
        .left_join(limits, col("id"), col("id"))
        .filter(col("Exit").is_not_null().or(col("Limit").is_not_null()))
        .collect()
        .map_err(|e| format!("Failed to build trade summary: {e}"))?;

    // If the joins/filtering still produce nothing, return the empty schema.
    if out.height() == 0 {
        return Ok(empty_trades_output());
    }

    Ok(out)
}

/* ---------------- timestamp normalization -> i64 ms ---------------- */

fn to_epoch_ms_series(df: &DataFrame) -> Result<Series, String> {
    use DataType::*;
    let col = df
        .column("timestamp")
        .map_err(|e| format!("frame missing 'timestamp': {e}"))?;

    // Work with a Series
    let s = col.clone().as_series().unwrap().clone();

    let out = match s.dtype() {
        // Already i64 (assume ms)
        Int64 => s,
        // Datetime -> Int64(ms)
        Datetime(_, _) => s
            .cast(&Int64)
            .map_err(|e| format!("cast datetime -> i64(ms) failed: {e}"))?,
        // Date -> Datetime(ms) -> Int64(ms)
        Date => s
            .cast(&Datetime(TimeUnit::Milliseconds, None))
            .and_then(|s| s.cast(&Int64))
            .map_err(|e| format!("cast date -> i64(ms) failed: {e}"))?,
        // i32/u64/u32 etc.: assume seconds, scale to ms
        Int32 | UInt64 | UInt32 => {
            let i64s = s
                .cast(&Int64)
                .map_err(|e| format!("cast timestamp -> i64 failed: {e}"))?;
            let ca = i64s.i64().map_err(|e| format!("to i64 failed: {e}"))?;
            let v: Vec<Option<i64>> = ca.into_iter().map(|o| o.map(|t| t * 1000)).collect();
            Series::new("timestamp".into(), v)
        }
        other => {
            return Err(format!(
                "unsupported timestamp dtype for trades alignment: {other:?}"
            ))
        }
    };
    Ok(out)
}

/* -------- cast a value column to f64; rename to alias -------- */
fn col_as_f64_series(df: &DataFrame, col: &str, alias: &str) -> Result<Series, String> {
    use DataType::*;
    let s = df
        .column(col)
        .map_err(|_| format!("column '{col}' not found"))?
        .clone()
        .as_series()
        .unwrap()
        .clone();

    let mut out = match s.dtype() {
        Float64 => s,
        Float32 | Int64 | Int32 | UInt64 | UInt32 => s
            .cast(&Float64)
            .map_err(|e| format!("cast '{col}' to Float64 failed: {e}"))?,
        other => {
            return Err(format!(
                "unsupported dtype for trades: {other:?} (col '{col}')"
            ))
        }
    };
    out.rename(alias.into()); // PlSmallStr via Into
    Ok(out)
}

/* -------- build a (timestamp_ms, value_f64) two-column frame -------- */
fn slice_frame_ms(df: &DataFrame, value_col: &str, alias: &str) -> Result<DataFrame, String> {
    let mut ts = to_epoch_ms_series(df)?;
    ts.rename("timestamp".into());
    let val = col_as_f64_series(df, value_col, alias)?;
    DataFrame::new(vec![ts.into_column(), val.into_column()]).map_err(|e| e.to_string())
}

/* -------- choose master timeline (smallest median step) -------- */
fn choose_master_idx(tables: &[DataFrame]) -> usize {
    let mut best = 0usize;
    let mut best_step = i64::MAX;
    for (i, t) in tables.iter().enumerate() {
        if let Ok(ca) = t.column("timestamp").and_then(|s| s.i64()) {
            let mut prev: Option<i64> = None;
            let mut diffs: Vec<i64> = Vec::new();
            for v in ca.into_iter().flatten() {
                if let Some(p) = prev {
                    let d = v - p;
                    if d > 0 {
                        diffs.push(d);
                    }
                }
                prev = Some(v);
            }
            if !diffs.is_empty() {
                diffs.sort_unstable();
                let median = diffs[diffs.len() / 2];
                if median < best_step {
                    best_step = median;
                    best = i;
                }
            }
        }
    }
    best
}

/* -------- manual backward as-of align to master with tolerance -------- */
fn align_to_master_backward(
    master: &DataFrame,
    others: &[(DataFrame, String)], // (df, value_col_name)
    tolerance_ms: i64,
) -> Result<DataFrame, String> {
    // Master timestamps
    let mts = master
        .column("timestamp")
        .map_err(|e| format!("master missing timestamp: {e}"))?
        .i64()
        .map_err(|e| format!("master timestamp not i64: {e}"))?;
    let mlen = mts.len();

    // Start with master as base (unique/sorted)
    let mut acc = master
        .clone()
        .lazy()
        .unique_stable(None, UniqueKeepStrategy::First)
        .sort(["timestamp"], SortMultipleOptions::default())
        .collect()
        .map_err(|e| format!("prepare master failed: {e}"))?;

    // For each “other” frame, produce a Series aligned by backward nearest within tolerance.
    for (df, val_name) in others {
        let ots = df
            .column("timestamp")
            .map_err(|e| format!("other missing timestamp: {e}"))?
            .i64()
            .map_err(|e| format!("other timestamp not i64: {e}"))?;
        let ocol = df
            .column(val_name)
            .map_err(|e| format!("missing aligned col '{val_name}': {e}"))?
            .f64()
            .map_err(|e| format!("aligned col '{val_name}' not f64: {e}"))?;

        // Build a simple two-pointer backward search using binary search
        // Assumes `df` is sorted by timestamp and without null ts.
        let mut out: Vec<Option<f64>> = Vec::with_capacity(mlen);
        // Build a Vec<(ts, val)> to binary_search on ts
        let mut ts_vec: Vec<i64> = Vec::with_capacity(ots.len());
        let mut val_vec: Vec<f64> = Vec::with_capacity(ots.len());
        for i in 0..ots.len() {
            if let (Some(t), Some(v)) = (ots.get(i), ocol.get(i)) {
                ts_vec.push(t);
                val_vec.push(v);
            }
        }

        for i in 0..mlen {
            let t = mts.get(i).unwrap_or(0);
            // find last index <= t
            match ts_vec.binary_search(&t) {
                Ok(idx) => out.push(Some(val_vec[idx])),
                Err(ins) => {
                    if ins == 0 {
                        out.push(None);
                    } else {
                        let j = ins - 1;
                        if t - ts_vec[j] <= tolerance_ms {
                            out.push(Some(val_vec[j]));
                        } else {
                            out.push(None);
                        }
                    }
                }
            }
        }

        let mut series = Series::new(val_name.clone().into(), out);
        // Append to acc
        acc = acc
            .hstack(&[series.into_column()])
            .map_err(|e| format!("hstack failed: {e}"))?;
    }

    // Drop rows where *all* aligned value columns are null (keep master ts where anything matched)
    let value_cols: Vec<&str> = acc
        .get_column_names()
        .into_iter()
        .filter(|n| *n != "timestamp")
        .map(|n| n as &str)
        .collect();

    if !value_cols.is_empty() {
        let mut pred = col(value_cols[0]).is_not_null();
        for n in &value_cols[1..] {
            pred = pred.or(col(n as &str)).is_not_null();
        }
        acc = acc
            .lazy()
            .filter(pred)
            .collect()
            .map_err(|e| format!("final filter failed: {e}"))?;
    }

    Ok(acc)
}

/* -------- align entry+exit series and return aligned table + aliases -------- */
fn align_all_by_timestamp_backward(
    entry_keys: &[(String, String)],
    exit_keys: &[(String, String)],
    frames: &HashMap<String, DataFrame>,
    tolerance_ms: i64,
) -> Result<(DataFrame, Vec<String>, Vec<String>), String> {
    // Build two-column tables & names
    let mut tables: Vec<DataFrame> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut entry_aliases: Vec<String> = Vec::new();
    let mut exit_aliases: Vec<String> = Vec::new();

    for (i, (fk, ck)) in entry_keys.iter().enumerate() {
        let alias = format!("E{i}");
        let f = frames
            .get(fk)
            .ok_or_else(|| format!("frame '{fk}' not found"))?;
        let mut t = slice_frame_ms(f, ck, &alias)?;
        t = t
            .lazy()
            .drop_nulls(None)
            .sort(["timestamp"], SortMultipleOptions::default())
            .collect()
            .map_err(|e| format!("prep entry '{fk}.{ck}' failed: {e}"))?;
        names.push(alias.clone());
        entry_aliases.push(alias);
        tables.push(t);
    }
    for (i, (fk, ck)) in exit_keys.iter().enumerate() {
        let alias = format!("X{i}");
        let f = frames
            .get(fk)
            .ok_or_else(|| format!("frame '{fk}' not found"))?;
        let mut t = slice_frame_ms(f, ck, &alias)?;
        t = t
            .lazy()
            .drop_nulls(None)
            .sort(["timestamp"], SortMultipleOptions::default())
            .collect()
            .map_err(|e| format!("prep exit '{fk}.{ck}' failed: {e}"))?;
        names.push(alias.clone());
        exit_aliases.push(alias);
        tables.push(t);
    }

    if tables.is_empty() {
        return Err("no series provided for alignment".into());
    }

    let master_idx = choose_master_idx(&tables);
    let master = tables.remove(master_idx);

    // Build (df, value_col) pairs for the others
    let mut others: Vec<(DataFrame, String)> = Vec::new();
    for (i, t) in tables.into_iter().enumerate() {
        // figure out its value column name (the non-timestamp col)
        let cname = t
            .get_column_names()
            .into_iter()
            .find(|n| *n != "timestamp")
            .ok_or_else(|| "aligned table missing value column".to_string())?
            .to_string();
        others.push((t, cname));
    }

    let aligned = align_to_master_backward(&master, &others, tolerance_ms)?;

    if aligned.height() == 0 {
        return Err(
            "no overlapping timestamps across entry/exit inputs after backward alignment".into(),
        );
    }

    Ok((aligned, entry_aliases, exit_aliases))
}

fn empty_trades_output() -> DataFrame {
    let id = Series::new("id".into(), Vec::<String>::new()); // Utf8 (empty)
    let entry = Series::new("Entry".into(), Vec::<i64>::new()); // Int64 (empty)
    let exit = Series::new("Exit".into(), Vec::<i64>::new()); // Int64 (empty)
    let limit = Series::new("Limit".into(), Vec::<i64>::new()); // Int64 (empty)
    DataFrame::new(vec![id.into(), entry.into(), exit.into(), limit.into()])
        .expect("empty trades schema")
}
