struct Sum { value: atomic<f32>; };
@group(0) @binding(0) var<storage, read>  src: array<f32>;
@group(0) @binding(1) var<storage, read_write> out1: array<f32>; // len=1; we just write out1[0]

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&src)) { return; }

    // Bit-cast to atomic<u32> to use atomics on fp via IEEE bits
    // (WGSL doesn't support atomic<f32> directly)
    // We'll do float atomic add via a CAS loop
    var newVal = src[i];

    // custom atomic add (float) at out1[0]
    loop {
        let oldBits = bitcast<u32>(out1[0]);
        let oldVal  = bitcast<f32>(oldBits);
        let sum     = oldVal + newVal;
        if (atomicCompareExchangeWeak(&bitcast<atomic<u32>>(out1[0]), oldBits, bitcast<u32>(sum)).exchanged) {
            break;
        }
    }
}
