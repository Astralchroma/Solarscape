struct PushConstants {
	camera: mat4x4<f32>,
	position_a: vec3<f32>,
    position_b: vec3<f32>,
    color: vec3<f32>,
}

var<push_constant> push_constants: PushConstants;

@vertex fn vertex(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    if vertex_index == 0 {
        return push_constants.camera * vec4(push_constants.position_a, 1.0);
    } else {
        return push_constants.camera * vec4(push_constants.position_b, 1.0);
    }
}

@fragment fn fragment() -> @location(0) vec4<f32> {
	return vec4<f32>(push_constants.color, 1.0);
}
