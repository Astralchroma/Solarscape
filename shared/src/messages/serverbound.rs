use nalgebra::Isometry3;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum ServerboundMessage {
	PlayerLocation(Isometry3<f32>),
}

impl From<Isometry3<f32>> for ServerboundMessage {
	fn from(location: Isometry3<f32>) -> Self {
		Self::PlayerLocation(location)
	}
}
