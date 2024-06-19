use crate::types::{ChunkCoordinates, Material, VoxjectId};
use nalgebra::Isometry3;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Clone, Deserialize, Serialize)]
pub struct AddVoxject {
	pub id: VoxjectId,
	pub name: Box<str>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct SyncVoxject {
	pub id: VoxjectId,
	pub location: Isometry3<f32>,
}

#[serde_as]
#[derive(Clone, Deserialize, Serialize)]
pub struct SyncChunk {
	pub coordinates: ChunkCoordinates,

	#[serde_as(as = "Box<[_; 4096]>")]
	pub materials: Box<[Material; 4096]>,

	#[serde_as(as = "Box<[_; 4096]>")]
	pub densities: Box<[f32; 4096]>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct RemoveChunk(pub ChunkCoordinates);

#[derive(Clone, Deserialize, Serialize)]
pub enum ClientboundMessage {
	AddVoxject(AddVoxject),
	SyncVoxject(SyncVoxject),
	SyncChunk(SyncChunk),
	RemoveChunk(RemoveChunk),
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

impl From<RemoveChunk> for ClientboundMessage {
	fn from(value: RemoveChunk) -> Self {
		Self::RemoveChunk(value)
	}
}
