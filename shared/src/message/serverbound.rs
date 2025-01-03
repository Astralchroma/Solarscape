use crate::data::world::{BlockType, Location};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum Serverbound {
	PlayerLocation(Location),
	GiveTestItem,
	CreateStructure(CreateStructure),
}

impl From<Location> for Serverbound {
	fn from(location: Location) -> Self {
		Self::PlayerLocation(location)
	}
}

/// Create a [Structure](crate::structure::Structure) at the specified [Location], with the specified [Block] at
/// 0, 0, 0.
#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct CreateStructure {
	pub location: Location,
	pub block: BlockType,
}

impl From<CreateStructure> for Serverbound {
	fn from(value: CreateStructure) -> Self {
		Self::CreateStructure(value)
	}
}
