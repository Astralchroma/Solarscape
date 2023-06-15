use crate::data::DisconnectReason;
use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum Clientbound {
	Hello,
	Disconnected(DisconnectReason),
}
