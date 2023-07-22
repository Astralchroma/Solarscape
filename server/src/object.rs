use crate::{chunk::Chunk, sector::Sector};
use nalgebra::Vector3;
use std::{
	collections::HashMap,
	sync::{atomic::Ordering::Relaxed, Arc},
};

pub const CHUNK_RADIUS: i32 = 2;
pub const RADIUS: f64 = (CHUNK_RADIUS << 4) as f64;

pub struct Object {
	pub object_id: u32,
	pub chunks: HashMap<Vector3<i32>, Chunk>,
}

impl Object {
	/// TODO: Temporary
	pub fn sphere(sector: &Arc<Sector>) -> Self {
		let mut star = Self {
			object_id: sector.object_id_counter.fetch_add(1, Relaxed),
			chunks: HashMap::new(),
		};
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
