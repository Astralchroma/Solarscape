use bincode::{Decode, Encode};
use std::sync::Arc;

#[derive(Debug, Decode, Encode)]
pub enum DisconnectReason {
	Disconnected,
	InternalError,
	ProtocolViolation,
	VersionMismatch(u16),
}

#[derive(Clone, Debug, Decode, Encode)]
pub struct SectorMeta {
	pub name: Arc<str>,
	pub display_name: Arc<str>,
}
