mod clientbound;
mod serverbound;

use bincode::{Decode, Encode};

pub use clientbound::*;
pub use serverbound::*;

#[derive(Debug, Decode, Encode)]
pub enum DisconnectReason {
	Disconnected,
	InternalError,
	ProtocolViolation,
	VersionMismatch(u16),
}
