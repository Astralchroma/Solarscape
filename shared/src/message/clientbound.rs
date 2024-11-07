use crate::data::{world::Block, world::ChunkCoordinates, world::Item, world::Location, world::Material, Id};
use crate::ShiftHasherBuilder;
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;

#[derive(Clone, Deserialize, Serialize)]
pub enum Clientbound {
	Sync(Sync),
	SyncInventory(SyncInventory),
	SyncChunk(SyncChunk),
	RemoveChunk(RemoveChunk),
	SyncStructure(SyncStructure),
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

/// Initial sync of a [Structure](crate::structure::Structure) when the Player logs in, the Structure is created, or
/// the Structure comes into view. This is not used for subsequent updates to the Structure.
#[derive(Clone, Deserialize, Serialize)]
pub struct SyncStructure {
	pub id: Id,
	pub location: Location,

	pub blocks: HashMap<Vector3<i16>, Block, ShiftHasherBuilder<3>>,
}

impl From<SyncStructure> for Clientbound {
	fn from(value: SyncStructure) -> Self {
		Self::SyncStructure(value)
	}
}
