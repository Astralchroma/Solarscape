use crate::chunk::Chunk;
use hecs::Entity;
use nalgebra::Vector3;

pub trait Generator {
	fn generate_chunk(&self, object: Entity, scale: u8, grid_position: Vector3<i32>) -> Chunk;
}

pub struct SphereGenerator {
	pub radius: f32,
}

impl Generator for SphereGenerator {
	fn generate_chunk(&self, object: Entity, scale: u8, grid_position: Vector3<i32>) -> Chunk {
		let mut chunk = Chunk::empty(object, scale, grid_position);
		let chunk_position = (grid_position * (16 << scale)).cast();
		let center_offset = Vector3::new(scale as f32 / 2.0, scale as f32 / 2.0, scale as f32 / 2.0);

		for x in (0..16 << scale).step_by(scale as usize + 1) {
			for y in (0..16 << scale).step_by(scale as usize + 1) {
				for z in (0..16 << scale).step_by(scale as usize + 1) {
					let cell_position = Vector3::new(x as f32, y as f32, z as f32) + center_offset + chunk_position;
					let distance = cell_position.metric_distance(&Vector3::new(0.0, 0.0, 0.0));

					chunk.set(
						&Vector3::new(x >> scale, y >> scale, z >> scale),
						distance < self.radius,
					);
				}
			}
		}

		chunk
	}
}
