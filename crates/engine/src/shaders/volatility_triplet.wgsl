// volatility_triplet.wgsl
struct Params {
  period: u32,
  annualize_flag: u32, // 0/1
  scale: f32,
  _pad0: f32,
}
@group(1) @binding(0) var<uniform> P: Params;

@group(0) @binding(0) var<storage, read> PRICE: array<f32>;
@group(0) @binding(1) var<storage, read_write> VOL: array<f32>;
@group(0) @binding(2) var<storage, read_write> POS: array<f32>;
@group(0) @binding(3) var<storage, read_write> NEG: array<f32>;

fn isnan_f(x: f32) -> bool { return x != x; }
fn f32_nan() -> f32 { return bitcast<f32>(0x7fc00000u); }

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= arrayLength(&VOL)) { return; }

  VOL[i] = f32_nan();
  POS[i] = f32_nan();
  NEG[i] = f32_nan();

  if (i + 1u < P.period) { return; }
  let start = i + 1u - P.period;

  // validate
  for (var j = start; j <= i; j++) {
    if (j == 0u) { return; }
    let p0 = PRICE[j - 1u];
    let p1 = PRICE[j];
    if (isnan_f(p0) || isnan_f(p1) || p0 <= 0.0 || p1 <= 0.0) { return; }
  }

  var n: u32 = 0u;
  var mean: f32 = 0.0;
  for (var j = start; j <= i; j++) {
    mean += log(PRICE[j] / PRICE[j - 1u]);
    n += 1u;
  }
  if (n < 2u) { return; }
  let nf = f32(n);
  mean = mean / nf;

  var var_acc: f32 = 0.0;
  for (var j = start; j <= i; j++) {
    let r = log(PRICE[j] / PRICE[j - 1u]);
    let d = r - mean;
    var_acc += d * d;
  }
  var std = sqrt(var_acc / (nf - 1.0));
  if (P.annualize_flag != 0u) { std = std * sqrt(252.0); }

  VOL[i] = std;

  let price = PRICE[i];
  if (!isnan_f(price)) {
    POS[i] = price * (1.0 + std * P.scale);
    NEG[i] = price * (1.0 - std * P.scale);
  }
}
