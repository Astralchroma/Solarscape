@group(0)
@binding(0)
var<uniform> camera: mat4x4<f32>;

struct ChunkData {
    @location(0)
    grid_position: vec3<f32>,
}

struct VertexData {
    @location(1)
    position: vec3<f32>,
}

struct FragmentData {
    @builtin(position)
    position: vec4<f32>,

    @location(0)
    distance: f32
}

@vertex
fn vertex(chunk: ChunkData, vertex: VertexData) -> FragmentData {
    var fragment_data: FragmentData;

    let position = chunk.grid_position + vertex.position;

    fragment_data.position = camera * vec4<f32>(position, 1.0);
    fragment_data.distance = distance(position, vec3<f32>(0.0));

    return fragment_data;
}

@fragment
fn fragment(input: FragmentData) -> @location(0) vec4<f32> {
    return vec4<f32>(vec3<f32>(input.distance / 100.0), 1.0);
}
