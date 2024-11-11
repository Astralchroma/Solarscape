struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) texture_coordinates: vec2<f32>,
}

struct Vertex {
	@builtin(position) position: vec4<f32>,
	@location(0) texture_coordinates: vec2<f32>,
}

struct PushConstants {
	camera: mat4x4<f32>,
	model: mat4x4<f32>,
}

var<push_constant> push_constants: PushConstants;

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

@vertex fn vertex(input: VertexInput) -> Vertex {
	var vertex: Vertex;

	vertex.position = push_constants.camera * push_constants.model * vec4(input.position, 1.0);
	vertex.texture_coordinates = input.texture_coordinates;

	return vertex;
}

@fragment fn fragment(vertex: Vertex) -> @location(0) vec4<f32> {
	return textureSample(texture, texture_sampler, vertex.texture_coordinates);
}
