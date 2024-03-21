@group(0) @binding(0) var<uniform> camera: mat4x4<f32>;

struct Chunk {
	@location(1) position: vec3<f32>,
	@location(2) scale: f32,
}

@vertex fn vertex(@location(0) position: vec3<f32>, chunk: Chunk) -> @builtin(position) vec4<f32> {
	return camera * vec4<f32>(chunk.position + (position * chunk.scale), 1.0);
}

@fragment fn fragment() -> @location(0) vec4<f32> {
	return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
