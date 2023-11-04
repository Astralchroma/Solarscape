use crate::chunk::CHUNK_VOLUME;
use bincode::{config::standard, Decode, Encode};
use hecs::Entity;
use nalgebra::Vector3;
use solarscape_macros::protocol_version;
use std::sync::Arc;

pub const PROTOCOL_VERSION: u16 = protocol_version!();

pub const PACKET_LENGTH_LIMIT: usize = 1 << 13;

#[derive(Decode, Encode)]
#[allow(clippy::large_enum_variant)] // Don't care
pub(crate) enum Protocol {
	Disconnected(DisconnectReason),
	Message(Message),
}

#[derive(Decode, Encode)]
#[allow(clippy::large_enum_variant)] // Don't care
pub enum Message {
	SyncEntity {
		#[bincode(with_serde)]
		entity: Entity,
		sync: SyncEntity,
	},
	Event(Event),
}

#[derive(Clone, Copy, Debug, Decode, Encode)]
pub enum DisconnectReason {
	ProtocolViolation,
	InternalError,
	ConnectionLost,
	Disconnected,
}

#[derive(Decode, Encode)]
#[allow(clippy::large_enum_variant)] // Don't care
pub enum SyncEntity {
	Sector {
		name: Box<str>,
		display_name: Box<str>,
	},
	Object,
	Chunk {
		#[bincode(with_serde)]
		grid_position: Vector3<i32>,

		data: [bool; CHUNK_VOLUME],
	},
}

#[derive(Decode, Encode)]
pub enum Event {
	ActiveSector(#[bincode(with_serde)] Entity),
	PositionUpdated(#[bincode(with_serde)] Vector3<f32>),
}

#[must_use]
pub fn encode(message: Message) -> Arc<[u8]> {
	bincode::encode_to_vec(Protocol::Message(message), standard())
		.expect("successful encode")
		.into()
}