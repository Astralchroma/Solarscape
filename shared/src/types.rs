use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{fmt::Display, fmt::Formatter};

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
#[must_use]
pub struct ChunkData {
	pub grid_coordinates: GridCoordinates,

	#[serde_as(as = "Box<[_; 4096]>")]
	pub materials: Box<[Material; 4096]>,

	#[serde_as(as = "Box<[_; 4096]>")]
	pub densities: Box<[f32; 4096]>,
}

impl ChunkData {
	pub fn new(grid_coordinates: GridCoordinates) -> Self {
		Self::from(grid_coordinates)
	}
}

impl From<GridCoordinates> for ChunkData {
	fn from(grid_coordinates: GridCoordinates) -> Self {
		Self { grid_coordinates, materials: Box::new([Material::Nothing; 4096]), densities: Box::new([0.0; 4096]) }
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[repr(u8)]
pub enum Material {
	Corium = 0b1100,
	Stone = 0b1101,
	Ground = 0b1110,

	Nothing = 0b1111,
}
