use nalgebra::Isometry3;

pub struct World {
	pub voxjects: Vec<Voxject>,
}

pub struct Voxject {
	pub name: String,
	pub position: Isometry3<f32>,
}
