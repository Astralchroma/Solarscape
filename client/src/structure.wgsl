struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) texture_coordinates: vec2<f32>,
}

struct InstanceInput {
	@location(2) model_a: vec4<f32>,
	@location(3) model_b: vec4<f32>,
	@location(4) model_c: vec4<f32>,
	@location(5) model_d: vec4<f32>,
	@location(6) opacity: f32,
}

struct Vertex {
	@builtin(position) position: vec4<f32>,
	@location(0) texture_coordinates: vec2<f32>,
	@location(1) opacity: f32,
}

var<push_constant> camera: mat4x4<f32>;

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

@vertex fn vertex(vertex: VertexInput, instance: InstanceInput) -> Vertex {
	let model = mat4x4(instance.model_a, instance.model_b, instance.model_c, instance.model_d);

	var output: Vertex;

	output.position = camera * model * vec4(vertex.position, 1.0);
	output.texture_coordinates = vertex.texture_coordinates;
	output.opacity = instance.opacity;

	return output;
}

@fragment fn fragment(vertex: Vertex) -> @location(0) vec4<f32> {
	return vec4(
		textureSample(texture, texture_sampler, vertex.texture_coordinates).xyz,
		vertex.opacity
	);
}
