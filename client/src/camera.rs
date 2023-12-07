use crate::renderer::Renderer;
use bytemuck::cast_slice;
use nalgebra::{dvector, Matrix4, Point3, Vector3};
use std::{mem, ops::Mul};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferUsages};
use winit::dpi::PhysicalPosition;
use winit::event::MouseScrollDelta::{self, LineDelta, PixelDelta};
use winit::event::{DeviceEvent, DeviceEvent::MouseMotion, ElementState, ElementState::Pressed, MouseButton};

pub struct Camera {
	position: Vector3<f64>,
	position_changed: bool,

	buffer: Buffer,
	pub bind_group: BindGroup,
}

impl Camera {
	#[must_use]
	pub fn new(renderer: &Renderer) -> Self {
		let buffer = renderer.device.create_buffer_init(&BufferInitDescriptor {
			label: Some("Camera.buffer"),
			contents: &[0; 80],
			usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
		});

		let bind_group = renderer.device.create_bind_group(&BindGroupDescriptor {
			label: Some("Camera.bind_group"),
			layout: &renderer.camera_bind_group_layout,
			entries: &[BindGroupEntry {
				binding: 0,
				resource: buffer.as_entire_binding(),
			}],
		});

		Self {
			position: Vector3::zeros(),
			position_changed: false,

			buffer,
			bind_group,
		}
	}

	pub fn update(&mut self, renderer: &Renderer, camera_controller: &OrbitCamera) {
		let new_position = camera_controller.get_position();
		if self.position != new_position {
			self.position = new_position;
			self.position_changed = true;
		}

		let aspect = renderer.size.width as f32 / renderer.size.height as f32;
		let projection = Matrix4::new_perspective(aspect, f32::to_radians(45.0), 0.0, f32::MAX);

		let view = Matrix4::look_at_rh(&Point3::origin(), &self.position.cast().into(), &Vector3::y());
		let matrix = projection * view;

		let mut buffer = [0; 80];
		buffer[0..64].copy_from_slice(cast_slice(matrix.as_slice()));
		buffer[64..76].copy_from_slice(cast_slice(self.position.cast::<f32>().as_slice()));

		renderer.queue.write_buffer(&self.buffer, 0, &buffer)
	}

	pub fn get_position(&self) -> &Vector3<f64> {
		&self.position
	}

	pub fn use_position_changed(&mut self) -> bool {
		mem::take(&mut self.position_changed)
	}
}

pub struct OrbitCamera {
	rotation_x: f64,
	rotation_y: f64,

	distance: f64,

	is_moving: ElementState,
}

impl OrbitCamera {
	pub fn get_position(&self) -> Vector3<f64> {
		Matrix4::from_euler_angles(self.rotation_y, self.rotation_x, 0.0)
			.mul(dvector![0.0, 0.0, self.distance, 0.0])
			.xyz()
	}

	pub fn handle_mouse_wheel(&mut self, event: MouseScrollDelta) {
		self.distance -= match event {
			LineDelta(_, y) => y as f64,
			PixelDelta(PhysicalPosition { y, .. }) => y,
		} * 2.0;

		if self.distance < 1.0 {
			self.distance = 1.0
		}
	}

	pub fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) {
		match button {
			MouseButton::Left => {}
			_ => return,
		};

		self.is_moving = state;
	}

	pub fn handle_device_event(&mut self, event: DeviceEvent) {
		match event {
			MouseMotion { delta: (x, y) } if Pressed == self.is_moving => {
				self.rotation_x += x / 500.0;
				self.rotation_y += y / 500.0;

				while self.rotation_y > f64::to_radians(89.0) {
					self.rotation_y = f64::to_radians(89.0);
				}

				while self.rotation_y < f64::to_radians(-89.0) {
					self.rotation_y = f64::to_radians(-89.0);
				}

				while self.rotation_x > f64::to_radians(360.0) {
					self.rotation_x -= f64::to_radians(360.0);
				}

				while self.rotation_x < f64::to_radians(0.0) {
					self.rotation_x += f64::to_radians(360.0);
				}
			}
			_ => {}
		}
	}
}

impl Default for OrbitCamera {
	fn default() -> Self {
		Self {
			rotation_x: 0.0,
			rotation_y: 0.0,
			distance: 64.0,
			is_moving: ElementState::Released,
		}
	}
}
