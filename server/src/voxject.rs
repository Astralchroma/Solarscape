use crate::Chunk;
use nalgebra::Vector3;
use std::collections::HashMap;

pub const CHUNK_RADIUS: i32 = 2;
pub const RADIUS: f64 = (CHUNK_RADIUS << 4) as f64;

pub struct Voxject {
	pub chunks: HashMap<Vector3<i32>, Chunk>,
}

impl Voxject {
	/// TODO: Temporary
	pub fn sphere() -> Self {
		let mut star = Self { chunks: HashMap::new() };
		star.populate_sphere();
		star
	}

	/// TODO: Temporary
	fn populate_sphere(&mut self) {
		for x in -CHUNK_RADIUS..CHUNK_RADIUS {
			for y in -CHUNK_RADIUS..CHUNK_RADIUS {
				for z in -CHUNK_RADIUS..CHUNK_RADIUS {
					let chunk_grid_position = Vector3::new(x, y, z);
					let chunk = Chunk::new_sphere(chunk_grid_position);
					self.chunks.insert(chunk_grid_position, chunk);
				}
			}
		}
	}
}
