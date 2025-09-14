use crate::parser::GraphSection;
use crate::parser::{DrawCommand, DrawType, Graph};
use polars::frame::DataFrame;
use std::collections::HashMap;

pub fn graph_over_data(
    graph_section: &GraphSection,
    frames: &HashMap<String, DataFrame>,
) -> Result<Graph, String> {
    let mut data: Vec<DrawType> = Vec::new();
    let mut axis_labels: Vec<String> = vec![];
    let xaxis = graph_section.xaxis.as_str();

    for command in &graph_section.commands {
        let df = frames
            .get(&command.get_frame())
            .ok_or_else(|| format!("Frame '{}' not found", command.get_frame()))?;

        // Extract timestamps for x-axis; fallback to sequential indices if missing
        let x_series = match extract_i64_column(df, "timestamp") {
            Ok(series) => series,
            Err(_) => (0..df.height()).map(|i| Some(i as i64)).collect(),
        };

        if axis_labels.is_empty() {
            x_series.iter().for_each(|x| {
                if let Some(value) = x {
                    axis_labels.push(format!("{}", value));
                } else {
                    axis_labels.push("N/A".to_string());
                }
            });
        }

        match command {
            DrawCommand::Line { name, series, .. } => {
                for field in series {
                    let values = extract_f64_column(df, field)?
                        .into_iter()
                        .map(|v| v.unwrap_or(0.0))
                        .collect();
                    data.push(DrawType::Line(
                        format!("{} - {}", command.get_frame(), name.clone()),
                        values,
                    ));
                }
            }

            DrawCommand::Bar { name, y, .. } => {
                let values = extract_f64_column(df, y)?
                    .into_iter()
                    .map(|v| v.unwrap_or(0.0))
                    .collect::<Vec<_>>();

                let x: Vec<f64> = x_series.iter().map(|x| x.unwrap_or(0) as f64).collect();
                data.push(DrawType::Bar(
                    format!("{} - {}", command.get_frame(), name.clone()),
                    x,
                ));
            }

            DrawCommand::Candle {
                name,
                open,
                high,
                low,
                close,
                ..
            } => {
                let open = extract_f64_column(df, open)?;
                let high = extract_f64_column(df, high)?;
                let low = extract_f64_column(df, low)?;
                let close = extract_f64_column(df, close)?;

                let candles = open
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

                data.push(DrawType::Candlestick(
                    format!("{} - {}", command.get_frame(), name.clone()),
                    candles,
                ));
            }
        }

        add_trade_rects(df, &x_series, &mut data)?;
    }

    Ok(Graph {
        data,
        axis_labels,
        title: "QQL Plot".into(),
    })
}

fn extract_f64_column(df: &DataFrame, name: &str) -> Result<Vec<Option<f64>>, String> {
    Ok(df
        .column(name)
        .map_err(|e| format!("Missing column '{}': {}", name, e))?
        .f64()
        .map_err(|e| format!("Column '{}' not f64: {}", name, e))?
        .to_vec())
}

fn extract_i64_column(df: &DataFrame, name: &str) -> Result<Vec<Option<i64>>, String> {
    Ok(df
        .column(name)
        .map_err(|e| format!("Missing x-axis column '{}': {}", name, e))?
        .i64()
        .map_err(|e| format!("x-axis column '{}' not i64: {}", name, e))?
        .to_vec())
}

fn extract_i32_column(df: &DataFrame, name: &str) -> Vec<Option<i32>> {
    df.column(name)
        .ok()
        .and_then(|s| s.i32().ok())
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

fn extract_f64_col(df: &DataFrame, name: &str) -> Vec<Option<f64>> {
    df.column(name)
        .ok()
        .and_then(|s| s.f64().ok())
        .map(|s| s.to_vec())
        .unwrap_or_default()
}

fn add_trade_rects(
    df: &DataFrame,
    timestamps: &[Option<i64>],
    data: &mut Vec<DrawType>,
) -> Result<(), String> {
    let entry = extract_i32_column(df, "entry");
    let exit = extract_i32_column(df, "exit");
    let limit = extract_i32_column(df, "limit");
    let close = extract_f64_col(df, "close");

    let mut trade_map: HashMap<i32, (usize, f64)> = HashMap::new();
    let mut green_rects = vec![];
    let mut red_rects = vec![];

    for i in 0..df.height() {
        if let Some(Some(id)) = entry.get(i) {
            let strike = close.get(i).unwrap_or(&Some(0.0)).unwrap_or(0.0);
            trade_map.insert(*id, (i, strike));
        }

        if let Some(Some(id)) = exit.get(i) {
            if let Some((start_idx, strike)) = trade_map.remove(id) {
                let x_start = timestamps[start_idx].unwrap_or(0) as f64;
                let x_end = timestamps[i].unwrap_or(0) as f64;
                green_rects.push((x_start, x_end, strike));
            }
        }

        if let Some(Some(id)) = limit.get(i) {
            if let Some((start_idx, strike)) = trade_map.remove(id) {
                let x_start = timestamps[start_idx].unwrap_or(0) as f64;
                let x_end = timestamps[i].unwrap_or(0) as f64;
                red_rects.push((x_start, x_end, strike));
            }
        }
    }

    if !green_rects.is_empty() {
        data.push(DrawType::GreenRect("Trades - Exits".into(), green_rects));
    }
    if !red_rects.is_empty() {
        data.push(DrawType::RedRect("Trades - Stops".into(), red_rects));
    }

    Ok(())
}
