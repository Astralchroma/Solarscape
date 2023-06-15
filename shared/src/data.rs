use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum DisconnectReason {
	Disconnected,
	InternalError,
	ProtocolViolation,
	VersionMismatch(u16),
}
