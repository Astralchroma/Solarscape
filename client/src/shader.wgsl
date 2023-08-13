@group(0)
@binding(0)
var<uniform> camera: mat4x4<f32>;

struct Vertex {
    @location(0)
    position: vec3<f32>,
}

@vertex
fn vertex(vertex: Vertex) -> @builtin(position) vec4<f32> {
    return camera * vec4<f32>(vertex.position, 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
