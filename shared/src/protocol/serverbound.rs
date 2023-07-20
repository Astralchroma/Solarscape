use crate::protocol::DisconnectReason;
use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum Serverbound {
	Hello { major_version: u16 },
	Disconnected { reason: DisconnectReason },
}
