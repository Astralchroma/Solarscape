use bincode::{Decode, Encode};
use nalgebra::Vector3;

pub const CHUNK_LENGTH: usize = 16;
pub const CHUNK_VOLUME: usize = usize::pow(CHUNK_LENGTH, 3);

#[derive(Copy, Clone, Debug, Decode, Encode)]
pub struct ChunkData {
	#[bincode(with_serde)]
	pub grid_position: Vector3<i32>,

	pub data: [bool; CHUNK_VOLUME],
}
