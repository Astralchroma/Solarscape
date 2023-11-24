use crate::{chunk::Chunk, components::Location, components::Sector, components::VoxelObject};
use bincode::{config::standard, Decode, Encode};
use hecs::Entity;
use nalgebra::Vector3;
use solarscape_macros::protocol_version;
use std::sync::Arc;

pub const PROTOCOL_VERSION: u16 = protocol_version!();

pub const PACKET_LENGTH_LIMIT: usize = 1 << 15;

#[derive(Decode, Encode)]
#[allow(clippy::large_enum_variant)] // Don't care
pub(crate) enum Protocol {
	Disconnected(DisconnectReason),
	Message(Message),
}

#[derive(Debug, Decode, Encode)]
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

#[derive(Debug, Decode, Encode)]
#[allow(clippy::large_enum_variant)] // Don't care
pub enum SyncEntity {
	Sector(Sector),
	VoxelObject(VoxelObject),
	Chunk(Chunk),
	Location(Location),
}

#[derive(Debug, Decode, Encode)]
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
