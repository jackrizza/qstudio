// volatility_vol_only.wgsl
struct Uniforms { period: u32, annualize: u32, _p0: u32, _p1: u32 };
@group(1) @binding(0) var<uniform> U: Uniforms;

@group(0) @binding(0) var<storage, read> price: array<f32>;
@group(0) @binding(1) var<storage, read_write> out_vol: array<f32>;

fn safe_sqrt(x: f32) -> f32 {
  if (x < 0.0) { return 0.0; }
  return sqrt(x);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i: u32 = gid.x;
  let n: u32 = U.period;
  let len: u32 = arrayLength(&price);
  if (i >= len) { return; }

  // not enough samples → 0.0 (or write NaN if you prefer and post-fill)
  if (n <= 1u || i + 1u < n) {
    out_vol[i] = 0.0;
    return;
  }

  var sum: f32 = 0.0;
  var sumsq: f32 = 0.0;
  let start: u32 = i + 1u - n;
  for (var k: u32 = 0u; k < n; k = k + 1u) {
    let x = price[start + k];
    sum += x;
    sumsq += x * x;
  }

  let nf: f32 = f32(n);
  let mean = sum / nf;
  let numer = sumsq - sum * sum / nf;
  // unbiased var, but guard denominator
  let denom = max(nf - 1.0, 1.0);
  var varv = numer / denom;

  // clamp tiny negatives to 0
  if (varv < 0.0) { varv = 0.0; }

  var vol = safe_sqrt(varv);
  if (U.annualize == 1u) {
    // scale to daily*sqrt(252) or whatever you need; here keep as raw
    // vol = vol * 15.874507; // e.g. sqrt(252) if your window is daily std of returns
  }
  out_vol[i] = vol;
}
