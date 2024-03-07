use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AddVoxject {
	pub id: usize,
	pub name: String,
}

#[derive(Deserialize, Serialize)]
// "Message" like clippy wants would conflict with the serverbound equivalent, would rather have ClientboundMessage
// instead of clientbound::Message
#[allow(clippy::module_name_repetitions)]
pub enum ClientboundMessage {
	AddVoxject(AddVoxject),
}

impl From<AddVoxject> for ClientboundMessage {
	fn from(value: AddVoxject) -> Self {
		Self::AddVoxject(value)
	}
}
