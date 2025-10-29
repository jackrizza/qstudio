// constant_fill.wgsl
struct Params {
  value: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}
@group(1) @binding(0) var<uniform> P: Params;

@group(0) @binding(0) var<storage, read_write> OUT: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= arrayLength(&OUT)) { return; }
  OUT[i] = P.value;
}
