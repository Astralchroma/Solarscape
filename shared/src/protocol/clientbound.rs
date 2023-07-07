use crate::{
	protocol::DisconnectReason,
	world::{sector::SectorData, voxject::ChunkData},
};
use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum Clientbound {
	Hello,
	Disconnected(DisconnectReason),
	SyncSector(SectorData),
	ActiveSector(usize),
	SyncChunk(ChunkData),
}
