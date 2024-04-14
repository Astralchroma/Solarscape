use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{fmt::Display, fmt::Formatter, ops::Add};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[must_use]
#[non_exhaustive]
pub struct GridCoordinates {
	pub coordinates: Vector3<i32>,
	pub level: u8,
}

impl GridCoordinates {
	/// # Panics
	/// If [`level`] is not 0..=31, as that would be out of bounds.
	pub fn new(coordinates: Vector3<i32>, level: u8) -> Self {
		assert!((0..=31).contains(&level));
		Self { coordinates, level }
	}

	/// # Panics
	/// If [`level`] is 31 as upleveled grid coordinates would be on level 32, which is out of bounds.
	pub fn upleveled(&self) -> Self {
		assert_ne!(self.level, 31);
		Self::new(self.coordinates.map(|coordinate| coordinate >> 1), self.level + 1)
	}

	/// # Panics
	/// If [`level`] is 0 as downleveled grid coordinates would be on level -1, which is out of bounds.
	pub fn downleveled(&self) -> Self {
		assert_ne!(self.level, 0);
		Self::new(self.coordinates.map(|coordinate| coordinate << 1), self.level - 1)
	}
}

impl Add<Vector3<i32>> for GridCoordinates {
	type Output = Self;

	fn add(mut self, rhs: Vector3<i32>) -> Self::Output {
		self.coordinates += rhs;
		self
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
	pub coordinates: GridCoordinates,

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
	fn from(coordinates: GridCoordinates) -> Self {
		Self { coordinates, materials: Box::new([Material::Nothing; 4096]), densities: Box::new([0.0; 4096]) }
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
