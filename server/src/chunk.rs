use crate::{
	connection::Connection,
	object::{Object, RADIUS},
};
use nalgebra::Vector3;
use solarscape_shared::{
	protocol::Clientbound,
	world::{chunk::index_of_vec, object::CHUNK_VOLUME},
};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

pub struct Chunk {
	pub object: Weak<Object>,
	pub grid_position: Vector3<i32>,
	pub data: RwLock<[bool; CHUNK_VOLUME]>,
}

impl Chunk {
	pub fn get(&self, cell_position: Vector3<u8>) -> bool {
		self.data.blocking_read()[index_of_vec(cell_position)]
	}

	pub fn set(&mut self, cell_position: Vector3<u8>, value: bool) {
		self.data.blocking_write()[index_of_vec(cell_position)] = value;
	}

	/// TODO: Temporary
	pub fn new_sphere(object: Weak<Object>, grid_position: Vector3<i32>) -> Self {
		let mut chunk = Self {
			object,
			grid_position,
			data: RwLock::new([false; CHUNK_VOLUME]),
		};
		chunk.populate_sphere();
		chunk
	}

	/// TODO: Temporary
	fn populate_sphere(&mut self) {
		let chunk_world_position = (self.grid_position * 16).cast();

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

	pub async fn subscribe(&self, connection: &Arc<Connection>) {
		let object = match self.object.upgrade() {
			Some(object) => object,
			None => return,
		};

		connection.send(Clientbound::SyncChunk {
			object_id: object.object_id,
			grid_position: self.grid_position,
			data: *self.data.read().await,
		})
	}
}
