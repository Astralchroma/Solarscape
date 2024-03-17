// The future plan for world generation is to have a FFI plugin system and a library for writing world generation
// plugins, for now though, we'll just do some basic world generation in here.
//
// When that happens, we'll probably start by just refactoring what's here into the library.

use crate::world::Chunk;
use nalgebra::Vector3;
use solarscape_shared::types::ChunkData;
use std::ops::{Deref, DerefMut};

pub struct ProtoChunk {
	level: u8,
	coordinates: Vector3<i32>,
	data: ChunkData,
}

impl Deref for ProtoChunk {
	type Target = ChunkData;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl DerefMut for ProtoChunk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
}

impl ProtoChunk {
	#[must_use]
	pub fn new(level: u8, coordinates: Vector3<i32>) -> Self {
		Self { level, coordinates, data: ChunkData::new() }
	}

	#[must_use]
	pub const fn level(&self) -> &u8 {
		&self.level
	}

	#[must_use]
	pub const fn coordinates(&self) -> &Vector3<i32> {
		&self.coordinates
	}

	/// Populates data with the distance to the center
	#[must_use]
	pub fn distance(mut self, center: Vector3<f32>) -> Self {
		let chunk_position = self.coordinates.cast();

		for x in 0..16 {
			for y in 0..16 {
				for z in 0..16 {
					let cell_position = Vector3::new(
						(x << self.level) as f32,
						(y << self.level) as f32,
						(z << self.level) as f32,
					);
					let position = chunk_position + cell_position;
					let distance = position.metric_distance(&center);

					self[x << 8 | y << 4 | z] = distance
				}
			}
		}

		self
	}

	/// Sets any cells exceeding `comparison` to the `new_value`
	#[must_use]
	pub fn set_greater_than(self, comparison: f32, new_value: f32) -> Self {
		self.map(|value| if value > comparison { new_value } else { value });
		self
	}

	#[must_use]
	pub fn build(self) -> Chunk {
		Chunk { level: self.level, coordinates: self.coordinates, data: self.data }
	}
}
