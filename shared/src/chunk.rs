use bincode::{Decode, Encode};
use hecs::Entity;
use nalgebra::Vector3;
use std::num::NonZeroU8;

pub const CHUNK_VOLUME: usize = usize::pow(16, 3);

#[derive(Clone, Copy, Debug, Decode, Encode)]
pub struct Chunk {
	/// The VoxelObject the Chunk belongs to
	#[bincode(with_serde)]
	pub voxel_object: Entity,

	/// The location of the Chunk on the Object grid
	#[bincode(with_serde)]
	pub grid_position: Vector3<i32>,

	/// Holds the type of level of chunk and references to any child chunks.
	pub octree_node: OctreeNode,

	pub density: [f32; CHUNK_VOLUME],
}

impl Chunk {
	pub fn empty(voxel_object: Entity, scale: u8, grid_position: Vector3<i32>) -> Self {
		Self {
			voxel_object,
			grid_position,
			octree_node: OctreeNode::new(scale),
			density: [0.0; CHUNK_VOLUME],
		}
	}

	pub fn get(&self, x: u8, y: u8, z: u8) -> f32 {
		self.density[index_of_u8(x, y, z)]
	}

	pub fn set(&mut self, cell_position: &Vector3<u8>, value: f32) {
		self.density[index_of_vec(cell_position)] = value;
	}
}

pub trait ChunkGet<T> {
	#[must_use]
	fn get(&mut self, cell_position: T) -> f32;
}

impl ChunkGet<Vector3<u8>> for Chunk {
	fn get(&mut self, cell_position: Vector3<u8>) -> f32 {
		self.density[index_of_u8(cell_position.x, cell_position.y, cell_position.z)]
	}
}

impl ChunkGet<[u8; 3]> for Chunk {
	fn get(&mut self, cell_position: [u8; 3]) -> f32 {
		self.density[index_of_u8(cell_position[0], cell_position[1], cell_position[2])]
	}
}

impl ChunkGet<(u8, u8, u8)> for Chunk {
	fn get(&mut self, cell_position: (u8, u8, u8)) -> f32 {
		self.density[index_of_u8(cell_position.0, cell_position.1, cell_position.2)]
	}
}

#[derive(Debug, Clone, Copy, Decode, Encode)]
pub enum OctreeNode {
	Real,
	Node {
		scale: NonZeroU8,

		#[bincode(with_serde)]
		children: Option<[Entity; 8]>,
	},
}

impl OctreeNode {
	#[must_use]
	pub const fn new(scale: u8) -> Self {
		match scale {
			0 => OctreeNode::Real,
			scale => OctreeNode::Node {
				// Doing it this way allows this function to be const.
				// We check that scale is not 0 ourself anyway, so the stdlib check becomes redundant.
				scale: unsafe { NonZeroU8::new_unchecked(scale) },
				children: None,
			},
		}
	}

	#[must_use]
	pub const fn scale(&self) -> u8 {
		match self {
			OctreeNode::Real => 0,
			OctreeNode::Node { scale, .. } => scale.get(),
		}
	}
}

#[must_use]
pub fn index_of_vec(cell_position: &Vector3<u8>) -> usize {
	let x = cell_position.x as usize;
	let y = cell_position.y as usize;
	let z = cell_position.z as usize;

	index_of(x, y, z)
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
