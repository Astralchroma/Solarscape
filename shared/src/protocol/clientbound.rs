use crate::{protocol::DisconnectReason, world::object::CHUNK_VOLUME};
use bincode::{Decode, Encode};
use nalgebra::Vector3;

#[derive(Debug, Decode, Encode)]
pub enum Clientbound {
	Disconnected {
		reason: DisconnectReason,
	},
	SyncSector {
		entity_id: u64,
		name: Box<str>,
		display_name: Box<str>,
	},
	ActiveSector {
		entity_id: u64,
	},
	AddObject {
		entity_id: u64,
	},
	SyncChunk {
		entity_id: u64,

		#[bincode(with_serde)]
		grid_position: Vector3<i32>,

		data: [bool; CHUNK_VOLUME],
	},
}
