use crate::{chunk::Chunk, connection::Connection, sync::Syncable};
use hecs::Entity;
use nalgebra::Vector3;
use solarscape_shared::protocol::Clientbound;
use std::collections::HashMap;

pub const CHUNK_RADIUS: i32 = 1;
pub const RADIUS: f64 = (CHUNK_RADIUS << 4) as f64 - 0.5;

pub struct Object {
	pub sector: Entity,
	pub chunks: HashMap<Vector3<i32>, Chunk>,
}

impl Object {
	/// TODO: Temporary
	pub fn sphere(sector: Entity) -> Self {
		let mut star = Self {
			sector,
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

impl Syncable for Object {
	fn sync(&self, connection: &mut Connection) {
		connection.send(Clientbound::AddObject { object_id: 0 });
		for chunk in self.chunks.values() {
			connection.send(Clientbound::SyncChunk {
				object_id: 0,
				grid_position: chunk.grid_position,
				data: chunk.data,
			})
		}
	}
}
