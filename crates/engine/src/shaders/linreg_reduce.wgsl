// linreg_reduce.wgsl
struct PC { len: u32, _pad0: u32, _pad1: u32, _pad2: u32 };

struct Reductions {
  n: atomic<u32>;
  sum_x: atomic<f32>;
  sum_y: atomic<f32>;
  sum_xx: atomic<f32>;
  sum_xy: atomic<f32>;
};

@group(0) @binding(0) var<storage, read> Y: array<f32>;
@group(0) @binding(1) var<storage, read_write> RED: Reductions;

fn isnan_f(x: f32) -> bool { return x != x; }

// Atomic add for f32 via bitcasts (WGSL doesn’t have atomic<f32> add; emulate with CAS)
// This is a standard pattern; fine for modest contention.
fn atomic_add_f32(dst: ptr<storage, atomic<u32>>, val: f32) {
  loop {
    let old_bits = atomicLoad(dst);
    let old = bitcast<f32>(old_bits);
    let new = old + val;
    let new_bits = bitcast<u32>(new);
    if (atomicCompareExchangeWeak(dst, old_bits, new_bits).exchanged) {
      break;
    }
  }
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @push_constant pc: PC) {
  let i = gid.x;
  if (i >= pc.len) { return; }
  let y = Y[i];
  if (isnan_f(y)) { return; }
  let xf = f32(i);

  // n += 1
  atomicAdd(&RED.n, 1u);
  // sum_x += x; sum_y += y; sum_xx += x*x; sum_xy += x*y
  atomic_add_f32(&RED.sum_x, xf);
  atomic_add_f32(&RED.sum_y, y);
  atomic_add_f32(&RED.sum_xx, xf * xf);
  atomic_add_f32(&RED.sum_xy, xf * y);
}
