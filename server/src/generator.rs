use crate::chunk::Chunk;
use hecs::Entity;
use nalgebra::Vector3;
use std::ops::Deref;

pub struct BoxedGenerator(Box<dyn Generator + Send + Sync>);

impl BoxedGenerator {
	pub fn new(generator: impl Generator + Send + Sync + 'static) -> Self {
		BoxedGenerator(Box::new(generator))
	}
}

impl Deref for BoxedGenerator {
	type Target = dyn Generator + Send + Sync;

	fn deref(&self) -> &Self::Target {
		&*self.0
	}
}

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

		for x in (0..16 << scale).step_by(scale as usize + 1) {
			for y in (0..16 << scale).step_by(scale as usize + 1) {
				for z in (0..16 << scale).step_by(scale as usize + 1) {
					let cell_position = Vector3::new(x as f32, y as f32, z as f32) + chunk_position;
					let distance = cell_position.metric_distance(&Vector3::new(0.0, 0.0, 0.0));

					chunk.set(&Vector3::new(x >> scale, y >> scale, z >> scale), distance);
				}
			}
		}

		chunk
	}
}
