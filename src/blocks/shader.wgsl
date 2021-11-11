[[block]]
struct Indices {
    data: array<f32>;
}; // this is used as both input and output for convenience

[[group(0), binding(0)]]
var<storage, read_write> v_indices: Indices;


[[stage(compute), workgroup_size(64)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    //var mult : u32 = 5;
    v_indices.data[global_id.x] = v_indices.data[global_id.x] * 12.0;
    //var n: u32 = v_indices.data[0];
    //var n: u32 = 5u;
    // var n: u32 = n_base;
    //v_indices.data[global_id.x] = 5.0;
}