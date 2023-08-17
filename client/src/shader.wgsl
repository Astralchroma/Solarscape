@group(0)
@binding(0)
var<uniform> camera: mat4x4<f32>;

struct VertexData {
    @location(0)
    position: vec3<f32>,
}

struct FragmentData {
    @builtin(position)
    position: vec4<f32>,

    @location(0)
    distance: f32
}

@vertex
fn vertex(vertex: VertexData) -> FragmentData {
    var fragment_data: FragmentData;

    fragment_data.position = camera * vec4<f32>(vertex.position, 1.0);
    fragment_data.distance = distance(vertex.position, vec3<f32>(0.0));

    return fragment_data;
}

@fragment
fn fragment(input: FragmentData) -> @location(0) vec4<f32> {
    return vec4<f32>(vec3<f32>(input.distance / 100.0), 1.0);
}
