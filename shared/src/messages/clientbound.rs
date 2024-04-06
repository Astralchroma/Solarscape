use crate::types::{ChunkData, GridCoordinates};
use nalgebra::Isometry3;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AddVoxject {
	pub voxject_index: usize,
	pub name: Box<str>,
}

#[derive(Deserialize, Serialize)]
pub struct SyncVoxject {
	pub voxject_index: usize,
	pub location: Isometry3<f32>,
}

#[derive(Deserialize, Serialize)]
pub struct SyncChunk {
	pub voxject_index: usize,
	pub grid_coordinates: GridCoordinates,
	pub data: ChunkData,
}

#[derive(Deserialize, Serialize)]
pub enum ClientboundMessage {
	AddVoxject(AddVoxject),
	SyncVoxject(SyncVoxject),
	SyncChunk(SyncChunk),
}

impl From<AddVoxject> for ClientboundMessage {
	fn from(value: AddVoxject) -> Self {
		Self::AddVoxject(value)
	}
}

impl From<SyncVoxject> for ClientboundMessage {
	fn from(value: SyncVoxject) -> Self {
		Self::SyncVoxject(value)
	}
}

impl From<SyncChunk> for ClientboundMessage {
	fn from(value: SyncChunk) -> Self {
		Self::SyncChunk(value)
	}
}
