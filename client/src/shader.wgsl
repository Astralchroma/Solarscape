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

    let camera_position = vec3<f32>(camera[0][3], camera[1][3], camera[2][3]);
    fragment_data.distance = distance(vertex.position, camera_position);

    return fragment_data;
}

@fragment
fn fragment(in: FragmentData) -> @location(0) vec4<f32> {
    // Temporary depth shading until we have something better
    return vec4<f32>(1.0) * pow(in.distance, 5.0) / pow(10.0, 6.3);
}
