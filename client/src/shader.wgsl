@group(0)
@binding(0)
var<uniform> camera: CameraData;

struct CameraData {
	projection: mat4x4<f32>,
	position: vec3<f32>
}

struct Location {
	@location(0)
	position: vec3<f32>,

	@location(1)
	rotation: vec3<f32>,

	@location(2)
	scale: f32,
}

struct Vertex {
	@location(3)
	position: vec3<f32>,
}

struct FragmentData {
	@builtin(position)
	position: vec4<f32>,

	@location(0)
	distance: f32
}

// TODO: Handle Rotation
@vertex
fn vertex(location: Location, vertex: Vertex) -> FragmentData {
	var fragment_data: FragmentData;

	let position = location.position + (vertex.position * vec3<f32>(location.scale)) + camera.position;

    fragment_data.position = camera.projection * vec4<f32>(position, 1.0);
	fragment_data.distance = distance(position, vec3<f32>(0.0));

	return fragment_data;
}

@fragment
fn fragment(input: FragmentData) -> @location(0) vec4<f32> {
	return vec4<f32>(vec3<f32>(input.distance / 100.0), 1.0);
}
