use crate::{protocol::DisconnectReason, world::SectorData};
use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum Clientbound {
	Hello,
	Disconnected(DisconnectReason),
	SyncSector(SectorData),
}
