use crate::connection::Connection;
use nalgebra::Isometry3;

pub struct Player {
	pub connection: Connection,
	pub position: Isometry3<f32>,
}