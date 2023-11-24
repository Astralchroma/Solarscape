use bytemuck::cast_slice;
use solarscape_shared::component::Location;
use std::ops::Deref;
use wgpu::{util::BufferInitDescriptor, util::DeviceExt, Buffer, BufferUsages, Device};

pub struct LocationBuffer(Buffer);

impl LocationBuffer {
	#[must_use]
	pub fn new(device: &Device, location: &Location) -> Self {
		Self(device.create_buffer_init(&BufferInitDescriptor {
			label: Some("LocationBuffer"),
			contents: &Self::encode(location),
			usage: BufferUsages::VERTEX,
		}))
	}

	#[must_use]
	fn encode(location: &Location) -> [u8; 28] {
		let mut byte_buffer = [0; 28];
		byte_buffer[0..12].copy_from_slice(cast_slice(location.position.cast::<f32>().as_slice()));
		byte_buffer[12..24].copy_from_slice(cast_slice(location.rotation.as_slice()));
		byte_buffer[24..].copy_from_slice(cast_slice(&[location.scale]));
		byte_buffer
	}
}

impl Deref for LocationBuffer {
	type Target = Buffer;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
