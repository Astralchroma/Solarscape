use crate::data::world::Location;
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

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct CreateStructure {
	pub location: Location,
}

impl From<CreateStructure> for Serverbound {
	fn from(value: CreateStructure) -> Self {
		Self::CreateStructure(value)
	}
}
