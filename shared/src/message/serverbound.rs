use nalgebra::IsometryMatrix3;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum Serverbound {
	PlayerLocation(IsometryMatrix3<f32>),
	GiveTestItem,
}

impl From<IsometryMatrix3<f32>> for Serverbound {
	fn from(location: IsometryMatrix3<f32>) -> Self {
		Self::PlayerLocation(location)
	}
}
