use nalgebra::{Isometry3, Vector3};
use std::collections::HashMap;

pub struct World {
	pub voxjects: Vec<Voxject>,
}

pub struct Voxject {
	pub name: String,
	pub location: Isometry3<f32>,
	pub chunks: [HashMap<Vector3<i32>, Chunk>; 31],
}

pub struct Chunk;
