use crate::protocol::DisconnectReason;
use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum Serverbound {
	Hello(u16),
	Disconnected(DisconnectReason),
}
