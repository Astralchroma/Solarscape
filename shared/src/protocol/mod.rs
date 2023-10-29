mod clientbound;
mod serverbound;

pub use clientbound::*;
pub use serverbound::*;

use bincode::{Decode, Encode};
use solarscape_macros::protocol_version;

#[derive(Debug, Decode, Encode)]
pub enum DisconnectReason {
	ConnectionLost,
	Disconnected,
	InternalError,
	ProtocolViolation,
	VersionMismatch(u16),
}

pub const PROTOCOL_VERSION: u16 = protocol_version!();

pub const PACKET_LENGTH_LIMIT: usize = 1 << 13;
