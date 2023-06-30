use bincode::{Decode, Encode};

#[derive(Debug, Decode, Encode)]
pub enum DisconnectReason {
	Disconnected,
	InternalError,
	ProtocolViolation,
	VersionMismatch(u16),
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct SectorData {
	pub name: Box<str>,
	pub display_name: Box<str>,
}
