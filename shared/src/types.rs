use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::ops::{Deref, DerefMut};

#[serde_as]
#[derive(Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct ChunkData(#[serde_as(as = "Box<[_; 4096]>")] Box<[f32; 4096]>);

impl Deref for ChunkData {
	type Target = [f32; 4096];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for ChunkData {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl Default for ChunkData {
	fn default() -> Self {
		Self(Box::new([0.0; 4096]))
	}
}

impl ChunkData {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}
}
