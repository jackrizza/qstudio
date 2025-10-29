// band_from_vol.wgsl
struct Uniforms { scale: f32, _p0: f32, _p1: f32, _p2: f32 };
@group(1) @binding(0) var<uniform> U: Uniforms;

@group(0) @binding(0) var<storage, read> price: array<f32>;
@group(0) @binding(1) var<storage, read> vol: array<f32>;
@group(0) @binding(2) var<storage, read_write> out_band: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i: u32 = gid.x;
  let len: u32 = arrayLength(&price);
  if (i >= len) { return; }

  var p = price[i];
  var v = vol[i];

  // replace non-finite with 0
  if (!(p == p)) { p = 0.0; } // NaN test
  if (!(v == v)) { v = 0.0; }

  out_band[i] = p * (1.0 + U.scale * v);
}
