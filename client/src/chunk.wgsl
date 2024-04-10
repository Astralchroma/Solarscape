struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
	@location(2) material_a: vec2<u32>,
	@location(3) material_b: vec2<u32>,
	@location(4) weight: f32,
}

struct Chunk {
	@location(5) position: vec3<f32>,
	@location(6) scale: f32,
}

struct Vertex {
	@builtin(position) position: vec4<f32>,
	@interpolate(linear) @location(0) chunk_position: vec3<f32>,
	@interpolate(linear) @location(1) normal: vec3<f32>,
	@location(2) material_a: vec2<u32>,
	@location(3) material_b: vec2<u32>,
	@interpolate(linear) @location(4) weight: f32,
}

@group(0) @binding(0) var<uniform> camera: mat4x4<f32>;

@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var texture_sampler: sampler;

@vertex fn vertex(input: VertexInput, chunk: Chunk) -> Vertex {
	var vertex: Vertex;

	vertex.position = camera * vec4<f32>(chunk.position + (input.position * chunk.scale), 1.0);
	vertex.chunk_position = input.position;
	vertex.normal = input.normal;
	vertex.material_a = input.material_a;
	vertex.material_b = input.material_b;
	vertex.weight = input.weight;

	return vertex;
}

fn get_color(material_coordinate: vec2<u32>, chunk_axis_position: vec2<f32>) -> vec4<f32> {
	let texture_coordinates = (vec2<f32>(material_coordinate) + fract(chunk_axis_position)) / 4;
	return textureSample(texture, texture_sampler, texture_coordinates);
}

@fragment fn fragment(vertex: Vertex) -> @location(0) vec4<f32> {
	let a_front = get_color(vertex.material_a, vertex.chunk_position.zy);
	let a_side = get_color(vertex.material_a, vertex.chunk_position.xy);
	let a_top = get_color(vertex.material_a, vertex.chunk_position.xz);

	let b_front = get_color(vertex.material_b, vertex.chunk_position.zy);
	let b_side = get_color(vertex.material_b, vertex.chunk_position.xy);
	let b_top = get_color(vertex.material_b, vertex.chunk_position.xz);

	var front = a_front + vertex.weight * (a_front - b_front);
	var side = a_side + vertex.weight * (a_side - b_side);
	var top = a_top + vertex.weight * (a_top - b_top);

	var weights = pow(abs(vertex.normal), vec3<f32>(1.0));
	weights = weights / (weights.x + weights.y + weights.z);

	front *= weights.x;
	side *= weights.z;
	top *= weights.y;

	return front + side + top;
}
