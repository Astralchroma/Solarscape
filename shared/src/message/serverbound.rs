use crate::data::world::Location;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum Serverbound {
	PlayerLocation(Location),
	GiveTestItem,
	CreateStructure,
}

impl From<Location> for Serverbound {
	fn from(location: Location) -> Self {
		Self::PlayerLocation(location)
	}
}
