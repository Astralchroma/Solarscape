use bincode::{Decode, Encode};
use hecs::Entity;
use nalgebra::Vector3;

pub const CHUNK_VOLUME: usize = usize::pow(16, 3);

#[derive(Clone, Copy, Debug, Decode, Encode)]
pub struct Chunk {
	#[bincode(with_serde)]
	pub voxel_object: Entity,

	pub level: u8,

	#[bincode(with_serde)]
	pub grid_position: Vector3<i32>,

	pub density: [f32; CHUNK_VOLUME],
}

impl Chunk {
	#[must_use]
	pub fn voxel_object_relative_position(&self) -> Vector3<i32> {
		self.grid_position * (16 << self.level as i32)
	}

	#[must_use]
	pub fn get_density(&self, x: u8, y: u8, z: u8) -> f32 {
		self.density[index_of_u8(x, y, z)]
	}

	pub fn set_density(&mut self, x: u8, y: u8, z: u8, value: f32) {
		self.density[index_of_u8(x, y, z)] = value;
	}
}

pub trait ChunkExtra<T> {
	#[must_use]
	fn get_density(&self, cell_position: &T) -> f32;

	fn set_density(&mut self, cell_position: &T, value: f32);
}

impl ChunkExtra<Vector3<u8>> for Chunk {
	fn get_density(&self, cell_position: &Vector3<u8>) -> f32 {
		self.density[index_of_u8(cell_position.x, cell_position.y, cell_position.z)]
	}

	fn set_density(&mut self, cell_position: &Vector3<u8>, value: f32) {
		self.density[index_of_u8(cell_position.x, cell_position.y, cell_position.z)] = value;
	}
}

impl ChunkExtra<[u8; 3]> for Chunk {
	fn get_density(&self, cell_position: &[u8; 3]) -> f32 {
		self.density[index_of_u8(cell_position[0], cell_position[1], cell_position[2])]
	}

	fn set_density(&mut self, cell_position: &[u8; 3], value: f32) {
		self.density[index_of_u8(cell_position[0], cell_position[1], cell_position[2])] = value;
	}
}

impl ChunkExtra<(u8, u8, u8)> for Chunk {
	fn get_density(&self, cell_position: &(u8, u8, u8)) -> f32 {
		self.density[index_of_u8(cell_position.0, cell_position.1, cell_position.2)]
	}

	fn set_density(&mut self, cell_position: &(u8, u8, u8), value: f32) {
		self.density[index_of_u8(cell_position.0, cell_position.1, cell_position.2)] = value;
	}
}

#[must_use]
pub fn index_of_u8(x: u8, y: u8, z: u8) -> usize {
	index_of(x as usize, y as usize, z as usize)
}

#[must_use]
pub fn index_of(x: usize, y: usize, z: usize) -> usize {
	assert!(x <= 0xf);
	assert!(y <= 0xf);
	assert!(z <= 0xf);

	(x << 8) + (y << 4) + z
}
