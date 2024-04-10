struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) materials: vec4<u32>,
	@location(2) weight: f32,
}

struct Chunk {
	@location(3) position: vec3<f32>,
	@location(4) scale: f32,
}

struct Vertex {
	@builtin(position) position: vec4<f32>,
	@location(0) chunk_position: vec2<f32>,
	@location(1) materials: vec4<u32>,
	@location(2) weight: f32,
}

@group(0) @binding(0) var<uniform> camera: mat4x4<f32>;

@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var texture_sampler: sampler;

@vertex fn vertex(input: VertexInput, chunk: Chunk) -> Vertex {
	var vertex: Vertex;

	vertex.position = camera * vec4<f32>(chunk.position + (input.position * chunk.scale), 1.0);
	vertex.chunk_position = input.position.xz;
	vertex.materials = input.materials;
	vertex.weight = input.weight;

	return vertex;
}

@fragment fn fragment(vertex: Vertex) -> @location(0) vec4<f32> {
	let texture_a = (vec2<f32>(vertex.materials.xy) + fract(vertex.chunk_position)) / 2;
	let texture_b = (vec2<f32>(vertex.materials.zw) + fract(vertex.chunk_position)) / 2;

	let color_a = textureSample(texture, texture_sampler, texture_a);
	let color_b = textureSample(texture, texture_sampler, texture_b);

	return color_a + vertex.weight * (color_b - color_a);
}
