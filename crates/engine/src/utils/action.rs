use crate::lexer::Keyword;
use crate::runtime::GpuDType;
use crate::runtime::GpuRuntime;
use crate::runtime::KernelStep;
use crate::runtime::OutputSpec;
use polars::prelude::FillNullStrategy;
use polars::prelude::*;
use std::borrow::Cow;

use polars::frame::DataFrame;
use polars::series::IsSorted;

use crate::calculation::Calculation;
use crate::parser::{ActionSection, Calc};

pub fn action_over_data_gpu(
    action: &ActionSection,
    df: DataFrame,
    rt: &mut GpuRuntime,
) -> Result<DataFrame, String> {
    pollster::block_on(async move {
        // Base: timestamp + selected fields
        let mut base = Vec::new();
        let ts = df
            .column("timestamp")
            .map_err(|e| format!("Failed to get timestamp column: {e}"))?
            .clone();
        base.push(ts);

        let selected = df
            .select(action.fields.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .map_err(|e| format!("Failed to select fields: {e}"))?;
        base.extend_from_slice(selected.get_columns());

        let Some(calcs) = &action.calc else {
            return DataFrame::new(base).map_err(|e| format!("Failed to create DataFrame: {e}"));
        };

        // let mut rt = GpuRuntime::new().await.map_err(|e| e.to_string())?;
        let mut working_df = df.clone();

        // sanitize core prices once
        let core: Vec<&str> = ["open", "high", "low", "close"]
            .into_iter()
            .filter(|c| has_col(&working_df, c))
            .collect();
        sanitize_for_gpu(&mut working_df, &core).map_err(|e| format!("sanitize: {e}"))?;

        let mut out_df =
            DataFrame::new(base).map_err(|e| format!("Failed to create DataFrame: {e}"))?;

        // process sorted calcs
        for calc in calcs {
            // upload all current columns except timestamp
            let mut fields = working_df.clone();
            // up front
            let core: Vec<&str> = ["open", "high", "low", "close"]
                .into_iter()
                .filter(|c| df.column(c).is_ok())
                .collect();
            sanitize_for_gpu(&mut fields, &core).map_err(|e| format!("sanitize: {e}"))?;

            let fields: Vec<&str> = fields
                .get_column_names()
                .into_iter()
                .filter(|n| !n.as_str().eq_ignore_ascii_case("timestamp"))
                .map(|s| s.as_str())
                .collect();

            // sanitize only this calc's inputs (coerce, fill, sort)
            let mut needed: Vec<&str> = calc.inputs.iter().map(|s| s.as_str()).collect();
            needed.retain(|c| *c != "timestamp");

            sanitize_for_gpu(&mut working_df, &needed).map_err(|e| format!("sanitize: {e}"))?;
            let mut table = rt
                .upload_dataframe(&working_df, Some(&fields.clone()))
                .map_err(|e| format!("GPU upload failed: {e}"))?;

            match calc.operation {
                Keyword::Constant => {
                    #[repr(C)]
                    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
                    struct Params {
                        value: f32,
                        _p0: f32,
                        _p1: f32,
                        _p2: f32,
                    }
                    let val = calc
                        .inputs
                        .get(0)
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(0.0);
                    let uniform = bytemuck::bytes_of(&Params {
                        value: val,
                        _p0: 0.0,
                        _p1: 0.0,
                        _p2: 0.0,
                    })
                    .to_vec();

                    let step = KernelStep {
                        shader_key: Cow::Borrowed("constant_fill"),
                        wgsl_src: Some(Cow::Borrowed(include_str!(
                            "../shaders/constant_fill.wgsl"
                        ))),
                        entry_point: Cow::Borrowed("main"),
                        inputs: vec![],
                        outputs: vec![OutputSpec::column(calc.alias.clone(), GpuDType::F32)],
                        push_constants: None,
                        workgroup_size_x: 256,
                        elems_per_invocation: 1,
                        uniform_bytes: Some(uniform),
                    };
                    rt.run_pipeline(&mut table, &[step])
                        .map_err(|e| e.to_string())?;

                    for df_mut in [&mut out_df, &mut working_df] {
                        rt.download_append(df_mut, &table, &calc.alias)
                            .map_err(|e| e.to_string())?;
                        cast_col(df_mut, &calc.alias, DataType::Float64)?;
                    }
                }

                Keyword::Difference => {
                    for (idx, w) in calc.inputs.windows(2).enumerate() {
                        let a = &w[0];
                        let b = &w[1];
                        let out_name = if calc.inputs.len() == 2 {
                            calc.alias.clone()
                        } else {
                            format!("{}_{}", calc.alias, idx)
                        };
                        let step = KernelStep {
                            shader_key: Cow::Borrowed("difference_pair"),
                            wgsl_src: Some(Cow::Borrowed(include_str!(
                                "../shaders/difference_pair.wgsl"
                            ))),
                            entry_point: Cow::Borrowed("main"),
                            inputs: vec![Cow::Owned(a.clone()), Cow::Owned(b.clone())],
                            outputs: vec![OutputSpec::column(out_name.clone(), GpuDType::F32)],
                            push_constants: None,
                            workgroup_size_x: 256,
                            elems_per_invocation: 1,
                            uniform_bytes: None,
                        };
                        rt.run_pipeline(&mut table, &[step])
                            .map_err(|e| e.to_string())?;
                        for df_mut in [&mut out_df, &mut working_df] {
                            rt.download_append(df_mut, &table, &out_name)
                                .map_err(|e| e.to_string())?;
                            cast_col(df_mut, &out_name, DataType::Float64)?;
                        }
                    }
                }

                Keyword::Sma => {
                    let src = calc
                        .inputs
                        .get(0)
                        .cloned()
                        .ok_or_else(|| "SMA requires one input column".to_string())?;
                    let period = calc
                        .inputs
                        .get(1)
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(14);
                    #[repr(C)]
                    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
                    struct Params {
                        period: u32,
                        _p0: u32,
                        _p1: u32,
                        _p2: u32,
                    }
                    let uniform = bytemuck::bytes_of(&Params {
                        period,
                        _p0: 0,
                        _p1: 0,
                        _p2: 0,
                    })
                    .to_vec();

                    let step = KernelStep {
                        shader_key: Cow::Borrowed("sma_centered"),
                        wgsl_src: Some(Cow::Borrowed(include_str!("../shaders/sma_centered.wgsl"))),
                        entry_point: Cow::Borrowed("main"),
                        inputs: vec![Cow::Owned(src.clone())],
                        outputs: vec![OutputSpec::column(calc.alias.clone(), GpuDType::F32)],
                        push_constants: None,
                        workgroup_size_x: 256,
                        elems_per_invocation: 1,
                        uniform_bytes: Some(uniform),
                    };
                    rt.run_pipeline(&mut table, &[step])
                        .map_err(|e| e.to_string())?;
                    for df_mut in [&mut out_df, &mut working_df] {
                        rt.download_append(df_mut, &table, &calc.alias)
                            .map_err(|e| e.to_string())?;
                        cast_col(df_mut, &calc.alias, DataType::Float64)?;
                    }
                }

                Keyword::Volatility | Keyword::DoubleVolatility => {
                    let price_col = calc
                        .inputs
                        .get(0)
                        .cloned()
                        .ok_or_else(|| "Volatility requires a price column".to_string())?;
                    let period = calc
                        .inputs
                        .get(1)
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(14);
                    let scale: f32 = if calc.operation == Keyword::Volatility {
                        0.5
                    } else {
                        1.0
                    };

                    #[repr(C)]
                    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
                    struct VolParams {
                        period: u32,
                        annualize: u32,
                        _p0: u32,
                        _p1: u32,
                    }
                    let vol_uniform = bytemuck::bytes_of(&VolParams {
                        period,
                        annualize: 1,
                        _p0: 0,
                        _p1: 0,
                    })
                    .to_vec();

                    let vol_name = calc.alias.clone();
                    let step_vol = KernelStep {
                        shader_key: Cow::Borrowed("volatility_vol_only"),
                        wgsl_src: Some(Cow::Borrowed(include_str!(
                            "../shaders/volatility_vol_only.wgsl"
                        ))),
                        entry_point: Cow::Borrowed("main"),
                        inputs: vec![Cow::Owned(price_col.clone())],
                        outputs: vec![OutputSpec::column(vol_name.clone(), GpuDType::F32)],
                        push_constants: None,
                        workgroup_size_x: 256,
                        elems_per_invocation: 1,
                        uniform_bytes: Some(vol_uniform),
                    };
                    rt.run_pipeline(&mut table, &[step_vol])
                        .map_err(|e| e.to_string())?;
                    for df_mut in [&mut out_df, &mut working_df] {
                        rt.download_append(df_mut, &table, &vol_name)
                            .map_err(|e| e.to_string())?;
                        cast_col(df_mut, &vol_name, DataType::Float64)?;
                    }

                    #[repr(C)]
                    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
                    struct BandParams {
                        scale: f32,
                        _p0: f32,
                        _p1: f32,
                        _p2: f32,
                    }

                    let pos_name = format!("{}_pos", vol_name);
                    let step_pos = KernelStep {
                        shader_key: Cow::Borrowed("band_from_vol"),
                        wgsl_src: Some(Cow::Borrowed(include_str!(
                            "../shaders/band_from_vol.wgsl"
                        ))),
                        entry_point: Cow::Borrowed("main"),
                        inputs: vec![Cow::Owned(price_col.clone()), Cow::Owned(vol_name.clone())],
                        outputs: vec![OutputSpec::column(pos_name.clone(), GpuDType::F32)],
                        push_constants: None,
                        workgroup_size_x: 256,
                        elems_per_invocation: 1,
                        uniform_bytes: Some(
                            bytemuck::bytes_of(&BandParams {
                                scale,
                                _p0: 0.0,
                                _p1: 0.0,
                                _p2: 0.0,
                            })
                            .to_vec(),
                        ),
                    };
                    rt.run_pipeline(&mut table, &[step_pos])
                        .map_err(|e| e.to_string())?;
                    for df_mut in [&mut out_df, &mut working_df] {
                        rt.download_append(df_mut, &table, &pos_name)
                            .map_err(|e| e.to_string())?;

                        // normalize_gpu_output(ut_df, &vol_name, DataType::Float64)?;
                        cast_col(df_mut, &pos_name, DataType::Float64)?;
                    }

                    let neg_name = format!("{}_neg", vol_name);
                    let step_neg = KernelStep {
                        shader_key: Cow::Borrowed("band_from_vol"),
                        wgsl_src: Some(Cow::Borrowed(include_str!(
                            "../shaders/band_from_vol.wgsl"
                        ))),
                        entry_point: Cow::Borrowed("main"),
                        inputs: vec![Cow::Owned(price_col.clone()), Cow::Owned(vol_name.clone())],
                        outputs: vec![OutputSpec::column(neg_name.clone(), GpuDType::F32)],
                        push_constants: None,
                        workgroup_size_x: 256,
                        elems_per_invocation: 1,
                        uniform_bytes: Some(
                            bytemuck::bytes_of(&BandParams {
                                scale: -scale,
                                _p0: 0.0,
                                _p1: 0.0,
                                _p2: 0.0,
                            })
                            .to_vec(),
                        ),
                    };
                    rt.run_pipeline(&mut table, &[step_neg])
                        .map_err(|e| e.to_string())?;
                    for df_mut in [&mut out_df, &mut working_df] {
                        rt.download_append(df_mut, &table, &neg_name)
                            .map_err(|e| e.to_string())?;
                        cast_col(df_mut, &neg_name, DataType::Float64)?;
                    }
                }

                Keyword::LinearRegression => {
                    // ensure LR inputs are f64
                    coerce_inputs_to_f64(&mut working_df, &calc.inputs)
                        .map_err(|e| format!("LinearRegression input coercion failed: {e}"))?;
                    let calculation = Calculation::new(calc.clone());
                    let cpu_df = calculation
                        .calculate(&working_df)
                        .map_err(|e| format!("LinearRegression CPU fallback failed: {e}"))?;
                    for col in cpu_df.get_columns() {
                        out_df
                            .with_column(col.clone())
                            .map_err(|e| format!("append col: {e}"))?;
                        working_df
                            .with_column(col.clone())
                            .map_err(|e| format!("append working col: {e}"))?;
                    }
                }

                _ => {
                    return Err(format!(
                        "Unsupported operation in GPU path: {:?}",
                        calc.operation
                    ))
                }
            }
        }

        Ok(out_df)
    })
}

// --- helpers ---

fn is_numeric_literal(s: &str) -> bool {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    if s.is_empty() {
        return false;
    }
    if s.chars()
        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
    {
        return false;
    }
    s.parse::<f64>().is_ok()
}

/// Coerce only the inputs required for THIS calc, skipping literals.
/// Returns Err only if a *non-literal* input column is truly missing or unsupported.
fn coerce_calc_inputs_to_f32_nan(df: &mut DataFrame, calc: &Calc) -> Result<(), String> {
    use polars::prelude::DataType;

    for name in &calc.inputs {
        if is_numeric_literal(name) {
            continue; // don't coerce literals
        }

        let s = df
            .column(name)
            .map_err(|_| format!("Required input column '{name}' not found"))?
            .clone();

        let s = match s.dtype() {
            DataType::Float32 => s,
            DataType::Float64
            | DataType::Int64
            | DataType::Int32
            | DataType::UInt64
            | DataType::UInt32 => s
                .cast(&DataType::Float32)
                .map_err(|e| format!("Failed to cast '{name}' to Float32: {e}"))?,
            other => {
                return Err(format!(
                    "Column '{name}' has unsupported dtype for GPU: {other:?}"
                ));
            }
        };

        // Fill nulls with NaN to keep length identical
        let ca = s.f32().unwrap();
        let filled: Vec<f32> = (0..ca.len())
            .map(|i| ca.get(i).unwrap_or(f32::NAN))
            .collect();
        let new_s = Series::new(name.as_str().into(), filled);

        df.replace(name, new_s)
            .map_err(|e| format!("Failed replacing column '{name}': {e}"))?;
    }

    Ok(())
}

fn coerce_needed_cols_to_f32_nan(df: &mut DataFrame, calcs: &[Calc]) -> Result<(), String> {
    use std::collections::HashSet;

    let mut needed: HashSet<String> = HashSet::new();
    for c in calcs {
        if !matches!(c.operation, Keyword::Constant) {
            for s in &c.inputs {
                needed.insert(s.clone());
            }
        }
    }

    for name in needed {
        let s = df
            .column(&name)
            .map_err(|_| format!("Required input column '{name}' not found"))?
            .clone();

        let s = match s.dtype() {
            DataType::Float32 => s,
            DataType::Float64
            | DataType::Int64
            | DataType::Int32
            | DataType::UInt64
            | DataType::UInt32 => s
                .cast(&DataType::Float32)
                .map_err(|e| format!("Failed to cast '{name}' to Float32: {e}"))?,
            other => {
                return Err(format!(
                    "Column '{name}' has unsupported dtype for GPU: {other:?}"
                ));
            }
        };

        // Fill nulls with NaN to keep length identical
        let ca = s.f32().unwrap();
        let filled: Vec<f32> = (0..ca.len())
            .map(|i| ca.get(i).unwrap_or(f32::NAN))
            .collect();
        let new_s = Series::new(name.as_str().into(), filled);

        df.replace(&name, new_s)
            .map_err(|e| format!("Failed replacing column '{name}': {e}"))?;
    }

    Ok(())
}

#[inline]
fn f32_to_bits(v: f32) -> u32 {
    v.to_bits()
}

pub fn action_over_data(action: &ActionSection, df: DataFrame) -> Result<DataFrame, String> {
    let mut df = df.clone();
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

        let calc_df = match calculation.calculate(&df) {
            Ok(result) => result,
            Err(e) => {
                log::error!("Failed to calculate: {}", e);
                return Err(format!("Failed to calculate: {}", e));
            }
        };
        // Append calc_df columns to the main DataFrame
        for col in calc_df.get_columns() {
            df.with_column(col.clone())
                .map_err(|e| format!("Failed to append column: {}", e))?;
        }
        columns.extend_from_slice(calc_df.get_columns());
    }

    let result_df =
        DataFrame::new(columns).map_err(|e| format!("Failed to create DataFrame: {}", e))?;
    Ok(result_df)
}

pub fn sanitize_for_gpu(df: &mut DataFrame, cols: &[&str]) -> Result<(), String> {
    // 1) ensure sorted by timestamp (if present)
    if let Ok(ts) = df.column("timestamp") {
        if matches!(ts.is_sorted_flag(), IsSorted::Not) {
            *df = df
                .clone()
                .lazy()
                .sort(["timestamp"], Default::default())
                .collect()
                .map_err(|e| format!("sort by timestamp failed: {e}"))?;
        }
    }

    // 2) cast to f32, 3) fill nulls, 4) replace non-finite
    // We do it column-by-column to avoid borrow issues.
    for &name in cols {
        if df.column(name).is_err() {
            continue;
        }

        // cast → f32
        let f32col = df
            .column(name)
            .unwrap()
            .clone()
            .cast(&DataType::Float32)
            .map_err(|e| format!("cast '{name}' to f32 failed: {e}"))?;

        // fill nulls (fwd then bwd)
        let filled = f32col
            .fill_null(FillNullStrategy::Forward(None))
            .map_err(|e| format!("fill_null fwd '{name}' failed: {e}"))?
            .fill_null(FillNullStrategy::Backward(None))
            .map_err(|e| format!("fill_null bwd '{name}' failed: {e}"))?;

        // replace non-finite (NaN/Inf) with neighbor values, then 0.0
        // pass 1: forward
        let cleaned_fwd = filled
            .clone()
            .f32()
            .ok()
            .map(|ca| {
                let mut v = ca.to_vec();
                for i in 0..v.len() {
                    if let Some(x) = v[i] {
                        if !x.is_finite() {
                            // find previous finite
                            let mut j = i.saturating_sub(1);
                            while i > 0 {
                                if let Some(px) = v[j] {
                                    if px.is_finite() {
                                        v[i] = Some(px);
                                        break;
                                    }
                                }
                                if j == 0 {
                                    break;
                                }
                                j -= 1;
                            }
                        }
                    }
                }
                Series::new(name.into(), v)
            })
            .unwrap_or(filled.as_series().unwrap().clone());

        // pass 2: backward
        let cleaned_bwd = cleaned_fwd
            .clone()
            .f32()
            .ok()
            .map(|ca| {
                let mut v = ca.to_vec();
                let n = v.len();
                for idx in (0..n).rev() {
                    if let Some(x) = v[idx] {
                        if !x.is_finite() {
                            // find next finite
                            let mut j = idx + 1;
                            while j < n {
                                if let Some(nx) = v[j] {
                                    if nx.is_finite() {
                                        v[idx] = Some(nx);
                                        break;
                                    }
                                }
                                j += 1;
                            }
                        }
                    }
                }
                // final pass: set any remaining None/non-finite to 0.0
                for x in v.iter_mut() {
                    if x.map(|y| !y.is_finite()).unwrap_or(true) {
                        *x = Some(0.0);
                    }
                }
                Series::new(name.into(), v)
            })
            .unwrap_or(cleaned_fwd);

        df.replace(name, cleaned_bwd)
            .map_err(|e| format!("replace '{name}' failed: {e}"))?;
    }

    Ok(())
}

// after GPU download, upcast to f64 for downstream consumers
fn cast_col(df: &mut DataFrame, name: &str, dtype: DataType) -> Result<(), String> {
    if !has_col(df, name) {
        return Ok(());
    }
    let casted = df
        .column(name)
        .map_err(|e| format!("get '{name}' failed: {e}"))?
        .clone()
        .cast(&dtype)
        .map_err(|e| format!("cast '{name}' failed: {e}"))?;
    df.with_column(casted)
        .map_err(|e| format!("with_column '{name}' failed: {e}"))?;
    Ok(())
}

fn coerce_inputs_to_f64_no_nan(df: &mut DataFrame, cols: &[String]) -> Result<(), String> {
    for name in cols {
        if df.column(name).is_err() {
            continue;
        }

        let cast = df
            .column(name)
            .unwrap()
            .clone()
            .cast(&DataType::Float64)
            .map_err(|e| format!("cast '{name}' to f64 failed: {e}"))?;

        // forward/back fill nulls
        let filled = cast
            .fill_null(FillNullStrategy::Forward(None))
            .map_err(|e| format!("fill fwd '{name}' failed: {e}"))?
            .fill_null(FillNullStrategy::Backward(None))
            .map_err(|e| format!("fill bwd '{name}' failed: {e}"))?;

        // replace non-finite with 0
        let v = filled
            .f64()
            .unwrap()
            .into_iter()
            .map(|o| {
                let x = o.unwrap_or(0.0);
                if x.is_finite() {
                    Some(x)
                } else {
                    Some(0.0)
                }
            })
            .collect::<Vec<_>>();

        df.replace(name, Series::new(name.as_str().into(), v))
            .map_err(|e| format!("replace '{name}' failed: {e}"))?;
    }
    Ok(())
}

// ensure LR inputs are Float64 (your Calculation expects f64)
fn coerce_inputs_to_f64(df: &mut DataFrame, inputs: &[String]) -> Result<(), String> {
    for name in inputs {
        if !has_col(df, name) {
            return Err(format!("LinearRegression input column '{name}' not found"));
        }
        let s64 = df
            .column(name)
            .map_err(|e| format!("get '{name}' failed: {e}"))?
            .clone()
            .cast(&DataType::Float64)
            .map_err(|e| format!("cast '{name}' to f64 failed: {e}"))?;
        df.with_column(s64)
            .map_err(|e| format!("with_column '{name}' failed: {e}"))?;
    }
    Ok(())
}

#[inline]
fn has_col(df: &DataFrame, name: &str) -> bool {
    df.get_column_names().iter().any(|n| n as &str == name)
}

fn normalize_gpu_output(df: &mut DataFrame, col: &str, want: DataType) -> Result<(), String> {
    // cast to f64 for graph/LR
    let s = df
        .column(col)
        .map_err(|_| format!("col '{col}' missing"))?
        .clone()
        .cast(&want)
        .map_err(|e| format!("cast '{col}' to {want:?} failed: {e}"))?;

    // replace NaN/Inf -> forward/back then zeros
    let mut s = s.f64().map_err(|e| format!("{col} not f64: {e}"))?.clone();
    // fill_null first (in case cast produced any)
    let mut si = s
        .clone()
        .into_series()
        .fill_null(FillNullStrategy::Forward(None))
        .map_err(|e| format!("fill_null fwd '{col}' failed: {e}"))?
        .fill_null(FillNullStrategy::Backward(None))
        .map_err(|e| format!("fill_null bwd '{col}' failed: {e}"))?
        .f64()
        .unwrap()
        .clone();

    // turn NaN/Inf into 0.0
    let v = si
        .into_iter()
        .map(|opt| {
            let x = opt.unwrap_or(0.0);
            if x.is_finite() {
                Some(x)
            } else {
                Some(0.0)
            }
        })
        .collect::<Vec<_>>();
    let out = Series::new(col.into(), v);

    df.replace(col, out)
        .map_err(|e| format!("replace '{col}' failed: {e}"))?;
    Ok(())
}
