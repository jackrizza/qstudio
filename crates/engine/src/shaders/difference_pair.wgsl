// difference_pair.wgsl
@group(0) @binding(0) var<storage, read> A: array<f32>;
@group(0) @binding(1) var<storage, read> B: array<f32>;
@group(0) @binding(2) var<storage, read_write> OUT: array<f32>;

fn isnan_f(x: f32) -> bool { return x != x; }
fn f32_nan() -> f32 { return bitcast<f32>(0x7fc00000u); }

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= arrayLength(&OUT)) { return; }

  let a = A[i];
  let b = B[i];

  if (isnan_f(a) || isnan_f(b)) {
    OUT[i] = f32_nan();
  } else {
    OUT[i] = a - b;
  }
}
