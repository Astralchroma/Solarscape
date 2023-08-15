@group(0)
@binding(0)
var<uniform> camera: mat4x4<f32>;

struct VertexInput {
    @location(0)
    position: vec3<f32>,
}

struct VertexOutput{
    @builtin(position) clip_position: vec4<f32>,
    @location(0) distance: f32
}

@vertex
fn vertex(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera * vec4<f32>(vertex.position, 1.0);
    out.distance = distance(
        vertex.position,
        // position from camera matrix
        vec3<f32>(camera[0][3], camera[1][3], camera[2][3])
    );
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // these numbers are somewhat arbitrary, just temporary shading
    return vec4<f32>(1.0) * pow(in.distance, 5.0) / pow(10.0, 6.3);
}
