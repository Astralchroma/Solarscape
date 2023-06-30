use crate::data::{DisconnectReason, SectorData};
use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum Clientbound {
	Hello,
	Disconnected(DisconnectReason),
	SyncSector(SectorData),
}
