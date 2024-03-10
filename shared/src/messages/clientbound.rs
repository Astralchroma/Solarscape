use nalgebra::{Isometry3, Vector3};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AddVoxject {
	pub id: usize,
	pub name: String,
}

#[derive(Deserialize, Serialize)]
pub struct VoxjectPosition {
	pub id: usize,
	pub position: Isometry3<f32>,
}

#[derive(Deserialize, Serialize)]
pub struct SyncChunk {
	pub voxject_id: usize,
	pub level: u8,
	pub grid_coordinate: Vector3<i32>,
}

#[derive(Deserialize, Serialize)]
// "Message" like clippy wants would conflict with the serverbound equivalent, would rather have ClientboundMessage
// instead of clientbound::Message
#[allow(clippy::module_name_repetitions)]
pub enum ClientboundMessage {
	AddVoxject(AddVoxject),
	VoxjectPosition(VoxjectPosition),
	SyncChunk(SyncChunk),
}

impl From<AddVoxject> for ClientboundMessage {
	fn from(value: AddVoxject) -> Self {
		Self::AddVoxject(value)
	}
}

impl From<VoxjectPosition> for ClientboundMessage {
	fn from(value: VoxjectPosition) -> Self {
		Self::VoxjectPosition(value)
	}
}

impl From<SyncChunk> for ClientboundMessage {
	fn from(value: SyncChunk) -> Self {
		Self::SyncChunk(value)
	}
}
