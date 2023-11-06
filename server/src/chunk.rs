use crate::{connection::ServerConnection, sync::Syncable};
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
	pub fn empty(object: Entity, grid_position: Vector3<i32>) -> Self {
		Self {
			object,
			grid_position,
			data: [false; CHUNK_VOLUME],
		}
	}

	pub fn get(&self, cell_position: &Vector3<u8>) -> bool {
		self.data[index_of_vec(cell_position)]
	}

	pub fn set(&mut self, cell_position: &Vector3<u8>, value: bool) {
		self.data[index_of_vec(cell_position)] = value;
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
