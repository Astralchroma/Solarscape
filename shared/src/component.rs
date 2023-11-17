use bincode::{Decode, Encode};
use hecs::Entity;

#[derive(Clone, Decode, Encode)]
pub struct Sector {
	/// The name as presented to the user
	pub name: Box<str>,

	/// The name as used in configuration
	pub display_name: Box<str>,
}

#[derive(Clone, Copy, Decode, Encode)]
pub struct Object {
	/// The Sector the Object belongs to
	#[bincode(with_serde)]
	pub sector: Entity,
}
