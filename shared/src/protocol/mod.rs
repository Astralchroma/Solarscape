mod clientbound;
mod serverbound;

pub use clientbound::*;
pub use serverbound::*;

use bincode::{Decode, Encode};
use once_cell::sync::Lazy;

#[derive(Debug, Decode, Encode)]
pub enum DisconnectReason {
	ConnectionLost,
	Disconnected,
	InternalError,
	ProtocolViolation,
	VersionMismatch(u16),
}

pub static PROTOCOL_VERSION: Lazy<u16> = Lazy::new(|| {
	env!("CARGO_PKG_VERSION_MAJOR")
		.parse()
		.expect("crate major version invalid")
});

pub const PACKET_LENGTH_LIMIT: usize = 1 << 13;
