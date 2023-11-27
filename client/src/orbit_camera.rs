use bytemuck::cast_slice;
use nalgebra::{dvector, Matrix4, Point3, Vector3};
use std::ops::Mul;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Buffer, BufferUsages, Device, Queue};
use winit::dpi::PhysicalPosition;
use winit::event::MouseScrollDelta::{self, LineDelta, PixelDelta};
use winit::event::{DeviceEvent, DeviceEvent::MouseMotion, ElementState, ElementState::Pressed, MouseButton};

const UP_VECTOR: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

pub struct OrbitCamera {
	rotation_x: f32,
	rotation_y: f32,

	distance: f32,

	is_moving: ElementState,

	pub position: Vector3<f32>,
	pub position_changed: bool,

	buffer: Buffer,
	pub bind: BindGroup,
}

impl OrbitCamera {
	pub fn new(device: &Device, bind_layout: &BindGroupLayout) -> Self {
		let buffer = device.create_buffer_init(&BufferInitDescriptor {
			label: Some("camera_buffer"),
			contents: &[0; 80],
			usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
		});

		let bind = device.create_bind_group(&BindGroupDescriptor {
			label: Some("camera_bind"),
			layout: bind_layout,
			entries: &[BindGroupEntry {
				binding: 0,
				resource: buffer.as_entire_binding(),
			}],
		});

		OrbitCamera {
			rotation_x: 0.0,
			rotation_y: 0.0,
			distance: 64.0,
			is_moving: ElementState::Released,
			position: Vector3::new(0.0, 0.0, 0.0),
			position_changed: true,
			buffer,
			bind,
		}
	}

	pub fn update_matrix(&self, queue: &Queue, width: u32, height: u32) {
		let aspect = width as f32 / height as f32;
		let projection = Matrix4::new_perspective(aspect, f32::to_radians(45.0), 0.0, f32::MAX);

		let view = Matrix4::look_at_rh(&Point3::origin(), &self.position.into(), &UP_VECTOR);
		let matrix = projection * view;

		let mut buffer = vec![];
		buffer.extend_from_slice(cast_slice(matrix.as_slice()));
		buffer.extend_from_slice(cast_slice(self.position.as_slice()));

		queue.write_buffer(&self.buffer, 0, &buffer)
	}

	pub fn update_position(&mut self) {
		self.position = Matrix4::from_euler_angles(self.rotation_y, self.rotation_x, 0.0)
			.mul(dvector![0.0, 0.0, self.distance, 0.0])
			.xyz();
		self.position_changed = true;
	}

	pub fn handle_mouse_wheel(&mut self, event: MouseScrollDelta) {
		self.distance -= match event {
			LineDelta(_, y) => y,
			PixelDelta(PhysicalPosition { y, .. }) => y as f32,
		} * 2.0;

		if self.distance < 1.0 {
			self.distance = 1.0
		}

		self.update_position();
	}

	pub fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) {
		match button {
			MouseButton::Left => {}
			_ => return,
		};

		self.is_moving = state;

		self.update_position();
	}

	pub fn handle_device_event(&mut self, event: DeviceEvent) {
		match event {
			MouseMotion { delta: (x, y) } if Pressed == self.is_moving => {
				self.rotation_x += x as f32 / 500.0;
				self.rotation_y += y as f32 / 500.0;

				while self.rotation_y > f32::to_radians(89.0) {
					self.rotation_y = f32::to_radians(89.0);
				}

				while self.rotation_y < f32::to_radians(-89.0) {
					self.rotation_y = f32::to_radians(-89.0);
				}

				while self.rotation_x > f32::to_radians(360.0) {
					self.rotation_x -= f32::to_radians(360.0);
				}

				while self.rotation_x < f32::to_radians(0.0) {
					self.rotation_x += f32::to_radians(360.0);
				}

				self.update_position();
			}
			_ => {}
		}
	}
}
