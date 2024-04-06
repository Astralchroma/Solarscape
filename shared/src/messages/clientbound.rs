use crate::types::ChunkData;
use nalgebra::Isometry3;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AddVoxject {
	pub voxject: usize,
	pub name: Box<str>,
}

#[derive(Deserialize, Serialize)]
pub struct SyncVoxject {
	pub voxject: usize,
	pub location: Isometry3<f32>,
}

#[derive(Deserialize, Serialize)]
pub struct SyncChunk {
	pub voxject: usize,
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
