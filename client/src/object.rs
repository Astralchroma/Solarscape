use crate::chunk::Chunk;
use nalgebra::Vector3;
use std::collections::HashMap;

pub struct Object {
	pub object_id: u32,
	pub chunks: HashMap<Vector3<i32>, Chunk>,
}

impl Object {
	pub fn new(object_id: u32) -> Self {
		Self {
			object_id,
			chunks: HashMap::new(),
		}
	}
}
