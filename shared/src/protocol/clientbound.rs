use crate::{protocol::DisconnectReason, world::voxject::CHUNK_VOLUME};
use bincode::{Decode, Encode};
use nalgebra::Vector3;

#[derive(Debug, Decode, Encode)]
pub enum Clientbound {
	Hello,
	Disconnected {
		reason: DisconnectReason,
	},
	SyncSector {
		name: Box<str>,
		display_name: Box<str>,
	},
	ActiveSector {
		network_id: usize,
	},
	SyncChunk {
		#[bincode(with_serde)]
		grid_position: Vector3<i32>,

		data: [bool; CHUNK_VOLUME],
	},
}
