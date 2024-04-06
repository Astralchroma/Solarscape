@group(0) @binding(0) var<uniform> camera: mat4x4<f32>;

struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) color: vec3<f32>,
}

struct Chunk {
	@location(2) position: vec3<f32>,
	@location(3) scale: f32,
}

struct Vertex {
	@builtin(position) position: vec4<f32>,
	@location(1) color: vec3<f32>,
}

@vertex fn vertex(input: VertexInput, chunk: Chunk) -> Vertex {
	var vertex: Vertex;

	vertex.position = camera * vec4<f32>(chunk.position + (input.position * chunk.scale), 1.0);
	vertex.color = input.color;

	return vertex;
}

@fragment fn fragment(vertex: Vertex) -> @location(0) vec4<f32> {
	return vec4<f32>(vertex.color, 1.0);
}
