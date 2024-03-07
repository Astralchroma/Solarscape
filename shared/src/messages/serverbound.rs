use nalgebra::Isometry3;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
// "Message" like clippy wants would conflict with the clientbound equivalent, would rather have ServerboundMessage
// instead of serverbound::Message
#[allow(clippy::module_name_repetitions)]
pub enum ServerboundMessage {
	PlayerPosition(Isometry3<f32>),
}

impl From<Isometry3<f32>> for ServerboundMessage {
	fn from(position: Isometry3<f32>) -> Self {
		Self::PlayerPosition(position)
	}
}
