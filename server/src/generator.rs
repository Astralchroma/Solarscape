use crate::chunk::Chunk;
use hecs::Entity;
use nalgebra::Vector3;

pub trait Generator {
	fn generate_chunk(&self, object: Entity, grid_position: Vector3<i32>) -> Chunk;
}

pub struct SphereGenerator {
	pub radius: f32,
}

impl Generator for SphereGenerator {
	fn generate_chunk(&self, object: Entity, grid_position: Vector3<i32>) -> Chunk {
		let mut chunk = Chunk::empty(object, grid_position);
		let chunk_position = (grid_position * 16).cast();

		for x in 0..16 {
			for y in 0..16 {
				for z in 0..16 {
					let cell_position = Vector3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) + chunk_position;
					let distance = cell_position.metric_distance(&Vector3::new(0.0, 0.0, 0.0));

					chunk.set(&Vector3::new(x, y, z), distance < self.radius);
				}
			}
		}

		chunk
	}
}