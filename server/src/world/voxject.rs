use nalgebra::Vector3;
use solarscape_shared::world::voxject::{ChunkData, CHUNK_VOLUME};
use std::collections::{hash_map::Values, HashMap};
use tokio::sync::RwLock;

const CHUNK_RADIUS: i32 = 2;
const RADIUS: f64 = (CHUNK_RADIUS << 4) as f64;

pub struct Voxject(HashMap<Vector3<i32>, Chunk>);

impl Voxject {
	pub fn get_chunk(&mut self, grid_position: Vector3<i32>) -> Option<&Chunk> {
		self.0.get(&grid_position)
	}

	pub fn chunks(&self) -> Values<'_, Vector3<i32>, Chunk> {
		self.0.values()
	}

	/// TODO: Temporary
	pub fn sphere() -> Self {
		let mut star = Self(HashMap::new());
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
					self.0.insert(chunk_grid_position, chunk);
				}
			}
		}
	}
}

pub struct Chunk(pub(crate) RwLock<ChunkData>);

impl Chunk {
	pub fn get(&self, cell_position: Vector3<u8>) -> bool {
		self.0.blocking_read().data[Self::index_of(cell_position)]
	}

	pub fn set(&mut self, cell_position: Vector3<u8>, value: bool) {
		self.0.blocking_write().data[Self::index_of(cell_position)] = value;
	}

	pub fn index_of(cell_position: Vector3<u8>) -> usize {
		let x = cell_position.x as usize;
		let y = cell_position.y as usize;
		let z = cell_position.z as usize;

		if x > 0xf {
			todo!("x should not exceed 15, it was {x}, this error should be handled properly.")
		}

		if y > 0xf {
			todo!("y should not exceed 15, it was {y}, this error should be handled properly.")
		}

		if z > 0xf {
			todo!("z should not exceed 15, it was {z}, this error should be handled properly.")
		}

		(x << 8) + (y << 4) + z
	}

	/// TODO: Temporary
	fn new_sphere(chunk_grid_position: Vector3<i32>) -> Self {
		let mut chunk = Self(RwLock::new(ChunkData {
			grid_position: chunk_grid_position,
			data: [false; CHUNK_VOLUME],
		}));
		chunk.populate_sphere();
		chunk
	}

	/// TODO: Temporary
	fn populate_sphere(&mut self) {
		let chunk_world_position = (self.0.blocking_read().grid_position * 16).cast();

		for x_i in 0..16 {
			let x_f = x_i as f64;

			for y_i in 0..16 {
				let y_f = y_i as f64;

				for z_i in 0..16 {
					let z_f = z_i as f64;

					let cell_chunk_position = Vector3::new(x_f, y_f, z_f);
					let cell_world_position = chunk_world_position + cell_chunk_position + Vector3::new(0.5, 0.5, 0.5);

					let distance = cell_world_position.metric_distance(&Vector3::new(0.0, 0.0, 0.0));

					let cell_position = Vector3::new(x_i, y_i, z_i);

					if distance < RADIUS {
						self.set(cell_position, true);
					} else {
						self.set(cell_position, false);
					}
				}
			}
		}
	}
}
