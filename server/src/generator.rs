use hecs::Entity;
use nalgebra::Vector3;
use solarscape_shared::chunk::{Chunk, ChunkGridPosition, CHUNK_VOLUME};
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
	fn generate_chunk(&self, voxel_object: Entity, scale: u8, grid_position: Vector3<i32>) -> Chunk;
}

pub struct SphereGenerator {
	pub radius: f32,
}

impl Generator for SphereGenerator {
	fn generate_chunk(&self, voxel_object: Entity, level: u8, grid_position: Vector3<i32>) -> Chunk {
		let mut chunk = Chunk {
			chunk_grid_position: ChunkGridPosition {
				voxel_object,
				level,
				grid_position,
			},
			density: [0.0; CHUNK_VOLUME],
		};
		let chunk_position = chunk.voxel_object_relative_position().cast();

		for x in (0..16 << level).step_by(level as usize + 1) {
			for y in (0..16 << level).step_by(level as usize + 1) {
				for z in (0..16 << level).step_by(level as usize + 1) {
					let cell_position = Vector3::new(x as f32, y as f32, z as f32) + chunk_position;
					let distance = cell_position.metric_distance(&Vector3::zeros());

					chunk.set_density(x >> level, y >> level, z >> level, self.radius - distance);
				}
			}
		}

		chunk
	}
}
