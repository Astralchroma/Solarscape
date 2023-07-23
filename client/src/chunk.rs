use nalgebra::Vector3;
use solarscape_shared::world::object::CHUNK_VOLUME;

pub struct Chunk {
	pub grid_position: Vector3<i32>,
	pub data: [bool; CHUNK_VOLUME],
}
