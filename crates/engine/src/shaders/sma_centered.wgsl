// sma_centered.wgsl
struct Params {
  period: u32,
  _pad0: u32,
  _pad1: u32,
  _pad2: u32,
}
@group(1) @binding(0) var<uniform> P: Params;

@group(0) @binding(0) var<storage, read> X: array<f32>;
@group(0) @binding(1) var<storage, read_write> OUT: array<f32>;

fn isnan_f(x: f32) -> bool { return x != x; }
fn f32_nan() -> f32 { return bitcast<f32>(0x7fc00000u); }

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  let len = arrayLength(&OUT);
  if (i >= len) { return; }
  OUT[i] = f32_nan();

  if (i + 1u < P.period) { return; }
  let start = i + 1u - P.period;

  var acc: f32 = 0.0;
  var count: u32 = 0u;
  for (var j = start; j <= i; j++) {
    let v = X[j];
    if (v == v) { acc += v; count += 1u; }
  }
  if (count < P.period) { return; }

  let sma = acc / f32(P.period);
  let shift = P.period / 2u;
  let dst = (i + shift) % len;
  OUT[dst] = sma;
}
