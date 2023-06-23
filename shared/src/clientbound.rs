use crate::data::{DisconnectReason, SectorMeta};
use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum Clientbound {
	Hello,
	Disconnected(DisconnectReason),
	UpdateSectorMeta(SectorMeta),
}
