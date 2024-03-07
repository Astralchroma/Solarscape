use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AddVoxject {
	pub id: usize,
	pub name: String,
}

#[derive(Deserialize, Serialize)]
pub enum Message {
	AddVoxject(AddVoxject),
}

impl From<AddVoxject> for Message {
	fn from(value: AddVoxject) -> Self {
		Self::AddVoxject(value)
	}
}
