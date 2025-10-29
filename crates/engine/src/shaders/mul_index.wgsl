@group(0) @binding(0) var<storage, read>  y:  array<f32>;
@group(0) @binding(1) var<storage, read_write> out_prod: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
let i = gid.x;
if (i >= arrayLength(&y)) { return; }
out_prod[i] = f32(i) * y[i];
}
