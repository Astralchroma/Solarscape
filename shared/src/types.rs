use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{fmt::Display, fmt::Formatter, ops::Deref, ops::DerefMut};

#[must_use]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct GridCoordinates {
	pub coordinates: Vector3<i32>,
	pub level: u8,
}

impl GridCoordinates {
	pub fn uplevel(&self) -> Self {
		Self { coordinates: self.coordinates.map(|coordinate| coordinate >> 1), level: self.level + 1 }
	}
}

impl Display for GridCoordinates {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}: {}, {}, {}",
			self.level, self.coordinates.x, self.coordinates.y, self.coordinates
		)
	}
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChunkData(#[serde_as(as = "Box<[_; 4096]>")] Box<[u8; 4096]>);

impl Deref for ChunkData {
	type Target = [u8; 4096];

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
		Self(Box::new([0; 4096]))
	}
}

impl ChunkData {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}
}
