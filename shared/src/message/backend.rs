use crate::types::Id;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AllowConnection {
	pub id: Id,
	pub key: [u8; 32],
}
