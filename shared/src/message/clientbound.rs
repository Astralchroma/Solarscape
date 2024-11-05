use crate::data::{world::ChunkCoordinates, world::Item, world::Material, Id};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Clone, Deserialize, Serialize)]
pub enum Clientbound {
	Sync(Sync),
	SyncInventory(SyncInventory),
	SyncChunk(SyncChunk),
	RemoveChunk(RemoveChunk),
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Sync {
	pub name: Box<str>,

	pub voxjects: Vec<Voxject>,

	pub inventory: Vec<InventorySlot>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Voxject {
	pub id: Id,
	pub name: Box<str>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "backend", derive(sqlx::Type))]
pub struct InventorySlot {
	pub item: Item,
	pub quantity: i64,
}

impl From<Sync> for Clientbound {
	fn from(value: Sync) -> Self {
		Self::Sync(value)
	}
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SyncInventory(pub Vec<InventorySlot>);

impl From<SyncInventory> for Clientbound {
	fn from(value: SyncInventory) -> Self {
		Self::SyncInventory(value)
	}
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

impl From<SyncChunk> for Clientbound {
	fn from(value: SyncChunk) -> Self {
		Self::SyncChunk(value)
	}
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct RemoveChunk(pub ChunkCoordinates);

impl From<RemoveChunk> for Clientbound {
	fn from(value: RemoveChunk) -> Self {
		Self::RemoveChunk(value)
	}
}
