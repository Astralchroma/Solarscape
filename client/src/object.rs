use crate::chunk::Chunk;
use nalgebra::Vector3;
use std::collections::HashMap;

#[derive(Default)]
pub struct Object {
	pub chunks: HashMap<Vector3<i32>, Chunk>,
}
