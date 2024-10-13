use crate::types::{ChunkCoordinates, Material, VoxjectId};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Clone, Deserialize, Serialize)]
pub struct SyncSector {
	pub name: Box<str>,

	pub voxjects: Vec<Voxject>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Voxject {
	pub id: VoxjectId,
	pub name: Box<str>,
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
pub enum Clientbound {
	SyncSector(SyncSector),
	SyncChunk(SyncChunk),
	RemoveChunk(RemoveChunk),
}

impl From<SyncSector> for Clientbound {
	fn from(value: SyncSector) -> Self {
		Self::SyncSector(value)
	}
}

impl From<SyncChunk> for Clientbound {
	fn from(value: SyncChunk) -> Self {
		Self::SyncChunk(value)
	}
}

impl From<RemoveChunk> for Clientbound {
	fn from(value: RemoveChunk) -> Self {
		Self::RemoveChunk(value)
	}
}
