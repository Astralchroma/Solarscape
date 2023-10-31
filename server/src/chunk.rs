use crate::{connection::ServerConnection, object::RADIUS, sync::Syncable};
use hecs::Entity;
use nalgebra::Vector3;
use solarscape_shared::chunk::{index_of_vec, CHUNK_VOLUME};
use solarscape_shared::protocol::{encode, Message, SyncEntity};

pub struct Chunk {
	pub object: Entity,
	pub grid_position: Vector3<i32>,
	pub data: [bool; CHUNK_VOLUME],
}

impl Chunk {
	pub fn new(object: Entity, grid_position: Vector3<i32>) -> Self {
		Self {
			object,
			grid_position,
			data: [false; CHUNK_VOLUME],
		}
	}

	pub fn get(&self, cell_position: Vector3<u8>) -> bool {
		self.data[index_of_vec(cell_position)]
	}

	pub fn set(&mut self, cell_position: Vector3<u8>, value: bool) {
		self.data[index_of_vec(cell_position)] = value;
	}

	/// TODO: Temporary
	pub fn generate_sphere_section(&mut self) {
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

					self.set(cell_position, distance < RADIUS);
				}
			}
		}
	}
}

impl Syncable for Chunk {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection) {
		connection.send(encode(Message::SyncEntity {
			entity,
			sync: SyncEntity::Chunk {
				grid_position: self.grid_position,
				data: self.data,
			},
		}))
	}
}
