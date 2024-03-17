@group(0) @binding(0) var<uniform> camera: mat4x4<f32>;

struct ChunkData {
	@location(1) r1: vec4<f32>,
	@location(2) r2: vec4<f32>,
	@location(3) r3: vec4<f32>,
	@location(4) r4: vec4<f32>,
}

@vertex fn vertex(@location(0) position: vec3<f32>, _chunk: ChunkData) -> @builtin(position) vec4<f32> {
	let chunk = mat4x4<f32>(_chunk.r1, _chunk.r2, _chunk.r3, _chunk.r4);

	return camera * chunk * vec4<f32>(position, 1.0);
}

@fragment fn fragment() -> @location(0) vec4<f32> {
	return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
