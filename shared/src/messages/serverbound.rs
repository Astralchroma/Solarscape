use nalgebra::Isometry3;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum ServerboundMessage {
	PlayerPosition(Isometry3<f32>),
}

impl From<Isometry3<f32>> for ServerboundMessage {
	fn from(position: Isometry3<f32>) -> Self {
		Self::PlayerPosition(position)
	}
}
