use crate::world::object::CHUNK_VOLUME;
use bincode::{Decode, Encode};
use nalgebra::Vector3;
use solarscape_macros::protocol_version;

pub const PROTOCOL_VERSION: u16 = protocol_version!();

pub const PACKET_LENGTH_LIMIT: usize = 1 << 13;

#[derive(Debug, Decode, Encode)]
pub enum DisconnectReason {
	ConnectionLost,
	Disconnected,
	InternalError,
	ProtocolViolation,
	VersionMismatch(u16),
}

#[derive(Debug, Decode, Encode)]
pub enum Serverbound {
	Hello { major_version: u16 },
	Disconnected { reason: DisconnectReason },
}

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
