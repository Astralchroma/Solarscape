use crate::data::Id;
use nalgebra::{vector, Point3, UnitQuaternion, Vector3};
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::{fmt, fmt::Display, fmt::Formatter, ops::Add, ops::Deref};

pub const LEVELS: u8 = 28;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Level(u8);

impl Level {
	pub const fn new(level: u8) -> Self {
		assert!(level < LEVELS, "out of bounds 0..=27");
		Self(level)
	}
}

impl<'d> Deserialize<'d> for Level {
	fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
		let level = u8::deserialize(deserializer)?;
		match level >= LEVELS {
			true => Err(Error::custom("out of bounds 0..=27")),
			false => Ok(Self(level)),
		}
	}
}

impl Display for Level {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl Deref for Level {
	type Target = u8;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChunkCoordinates {
	pub voxject: Id,
	pub coordinates: Vector3<i32>,
	pub level: Level,
}

impl ChunkCoordinates {
	pub const fn new(voxject: Id, coordinates: Vector3<i32>, level: Level) -> Self {
		Self {
			voxject,
			coordinates,
			level,
		}
	}

	/// # Panics
	/// If [`level`] is 27 as upleveled grid coordinates would be on level 28, which is out of bounds.
	pub fn upleveled(&self) -> Self {
		assert!(*self.level < LEVELS - 1);
		Self::new(
			self.voxject,
			self.coordinates.map(|coordinate| coordinate >> 1),
			Level::new(*self.level + 1),
		)
	}

	/// # Panics
	/// If [`level`] is 0 as downleveled grid coordinates would be on level -1, which is out of bounds.
	pub fn downleveled(&self) -> Self {
		assert_ne!(*self.level, 0);
		Self::new(
			self.voxject,
			self.coordinates.map(|coordinate| coordinate << 1),
			Level::new(*self.level - 1),
		)
	}

	/// Returns the Chunk's translation relative to the Voxject.
	pub fn voxject_relative_translation(&self) -> Vector3<f32> {
		self.coordinates.map(|coordinate| coordinate << *self.level).cast() * 16.0
	}

	/// Returns a list of the Chunk's surrounding chunks. These are both the Chunk's dependents and dependencies.
	/// Chunks are ordered from -1 to 1, x then y then z, this ordering can be relied on.
	#[rustfmt::skip]
	pub fn surrounding(&self) -> [ChunkCoordinates; 26] {
		[
			*self + vector![-1, -1, -1],
			*self + vector![-1, -1,  0],
			*self + vector![-1, -1,  1],
			*self + vector![-1,  0, -1],
			*self + vector![-1,  0,  0],
			*self + vector![-1,  0,  1],
			*self + vector![-1,  1, -1],
			*self + vector![-1,  1,  0],
			*self + vector![-1,  1,  1],
			*self + vector![ 0, -1, -1],
			*self + vector![ 0, -1,  0],
			*self + vector![ 0, -1,  1],
			*self + vector![ 0,  0, -1],
			*self + vector![ 0,  0,  1],
			*self + vector![ 0,  1, -1],
			*self + vector![ 0,  1,  0],
			*self + vector![ 0,  1,  1],
			*self + vector![ 1, -1, -1],
			*self + vector![ 1, -1,  0],
			*self + vector![ 1, -1,  1],
			*self + vector![ 1,  0, -1],
			*self + vector![ 1,  0,  0],
			*self + vector![ 1,  0,  1],
			*self + vector![ 1,  1, -1],
			*self + vector![ 1,  1,  0],
			*self + vector![ 1,  1,  1],
		]
	}

	/// Returns a list of the Chunk's surrounding chunks and the current chunks.
	/// Chunks are ordered from -1 to 1, x then y then z, this ordering can be relied on.
	#[rustfmt::skip]
	pub fn surrounding_and_current(&self) -> [ChunkCoordinates; 27] {
		[
			*self + vector![-1, -1, -1],
			*self + vector![-1, -1,  0],
			*self + vector![-1, -1,  1],
			*self + vector![-1,  0, -1],
			*self + vector![-1,  0,  0],
			*self + vector![-1,  0,  1],
			*self + vector![-1,  1, -1],
			*self + vector![-1,  1,  0],
			*self + vector![-1,  1,  1],
			*self + vector![ 0, -1, -1],
			*self + vector![ 0, -1,  0],
			*self + vector![ 0, -1,  1],
			*self + vector![ 0,  0, -1],
			*self,
			*self + vector![ 0,  0,  1],
			*self + vector![ 0,  1, -1],
			*self + vector![ 0,  1,  0],
			*self + vector![ 0,  1,  1],
			*self + vector![ 1, -1, -1],
			*self + vector![ 1, -1,  0],
			*self + vector![ 1, -1,  1],
			*self + vector![ 1,  0, -1],
			*self + vector![ 1,  0,  0],
			*self + vector![ 1,  0,  1],
			*self + vector![ 1,  1, -1],
			*self + vector![ 1,  1,  0],
			*self + vector![ 1,  1,  1],
		]
	}
}

impl Add<Vector3<i32>> for ChunkCoordinates {
	type Output = Self;

	fn add(mut self, rhs: Vector3<i32>) -> Self::Output {
		self.coordinates += rhs;
		self
	}
}

impl Display for ChunkCoordinates {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
		write!(
			formatter,
			"{}[{}]: {}, {}, {}",
			self.voxject, self.level, self.x, self.y, self.z
		)
	}
}

impl Deref for ChunkCoordinates {
	type Target = Vector3<i32>;

	fn deref(&self) -> &Self::Target {
		&self.coordinates
	}
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
pub struct Location {
	pub position: Point3<f32>,
	pub rotation: UnitQuaternion<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[repr(u8)]
pub enum Material {
	Corium = 0b1100,
	Stone = 0b1101,
	Ground = 0b1110,

	Nothing = 0b1111,
}

#[cfg_attr(feature = "backend", derive(sqlx::Type))]
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum Item {
	TestOre,
}

impl Item {
	pub const fn name(&self) -> &'static str {
		match self {
			Self::TestOre => "test_ore",
		}
	}

	pub const fn display_name(&self) -> &'static str {
		match self {
			Self::TestOre => "Test Ore",
		}
	}

	pub const fn description(&self) -> &'static str {
		match self {
			Self::TestOre => "A material so alien that it breaks reality",
		}
	}
}
