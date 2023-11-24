use bincode::{Decode, Encode};
use hecs::Entity;
use nalgebra::Vector3;

#[derive(Clone, Debug, Decode, Encode)]
pub struct Sector {
	/// The name as presented to the user
	pub name: Box<str>,

	/// The name as used in configuration
	pub display_name: Box<str>,
}

#[derive(Clone, Copy, Debug, Decode, Encode)]
pub struct VoxelObject {
	/// The Sector the VoxelObject belongs to
	#[bincode(with_serde)]
	pub sector: Entity,
}

#[derive(Clone, Copy, Debug, Decode, Encode)]
pub struct Location {
	/// Position is relative to the origin (0, 0, 0)
	#[bincode(with_serde)]
	pub position: Vector3<f64>,

	/// Stored in radians
	#[bincode(with_serde)]
	pub rotation: Vector3<f32>,

	#[bincode(with_serde)]
	pub scale: f32,
}
