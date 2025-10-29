struct Params { a: f32, b: f32, _pad0: f32, _pad1: f32 };
@group(1) @binding(0) var<uniform> U: Params;

@group(0) @binding(0) var<storage, read_write> out_yhat: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&out_yhat)) {
        return;
    }
    out_yhat[i] = U.a + U.b * f32(i);
}
