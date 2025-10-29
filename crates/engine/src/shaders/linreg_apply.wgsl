// linreg_apply.wgsl
struct PC { len: u32, _pad0: u32, _pad1: u32, _pad2: u32 };

struct Reductions {
  n: atomic<u32>;
  sum_x: atomic<u32>;  // read as bits
  sum_y: atomic<u32>;
  sum_xx: atomic<u32>;
  sum_xy: atomic<u32>;
};

@group(0) @binding(0) var<storage, read> RED: Reductions;
@group(0) @binding(1) var<storage, read_write> YHAT: array<f32>;

fn load_f(a: atomic<u32>) -> f32 { return bitcast<f32>(atomicLoad(&a)); }

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @push_constant pc: PC) {
  let i = gid.x;
  if (i >= pc.len) { return; }

  let n = atomicLoad(&RED.n);
  if (n < 2u) {
    YHAT[i] = f32.nan();
    return;
  }

  let nf = f32(n);
  let sx = load_f(RED.sum_x);
  let sy = load_f(RED.sum_y);
  let sxx = load_f(RED.sum_xx);
  let sxy = load_f(RED.sum_xy);

  // slope a = (n*sum_xy - sum_x*sum_y) / (n*sum_xx - sum_x*sum_x)
  let denom = nf * sxx - sx * sx;
  if (abs(denom) <= 1e-12) {
    YHAT[i] = f32.nan();
    return;
  }
  let a = (nf * sxy - sx * sy) / denom;
  let b = (sy - a * sx) / nf;

  YHAT[i] = a * f32(i) + b;
}
