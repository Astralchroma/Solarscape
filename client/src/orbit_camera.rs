use bytemuck::cast_slice;
use nalgebra::{dvector, Matrix4, Point3, Vector3};
use std::ops::{Add, Mul};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
	BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, Buffer, BufferUsages, Device, Queue, RenderPass,
};
use winit::dpi::PhysicalPosition;
use winit::event::MouseScrollDelta::{self, LineDelta, PixelDelta};
use winit::event::{DeviceEvent, ElementState, MouseButton};
use DeviceEvent::MouseMotion;
use ElementState::Pressed;

const UP_VECTOR: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

pub struct OrbitCamera {
	rotation_x: f32,
	rotation_y: f32,

	distance: f32,

	is_moving: ElementState,

	buffer: Buffer,
	bind: BindGroup,
}

impl OrbitCamera {
	pub fn new(device: &Device, bind_layout: &BindGroupLayout) -> Self {
		let buffer = device.create_buffer_init(&BufferInitDescriptor {
			label: Some("camera_buffer"),
			contents: &[0; 64],
			usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
		});

		let bind = device.create_bind_group(&BindGroupDescriptor {
			label: Some("camera_bind"),
			layout: &bind_layout,
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
			buffer,
			bind,
		}
	}

	pub fn use_camera<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		render_pass.set_bind_group(0, &self.bind, &[]);
	}

	pub fn update_matrix(&self, queue: &Queue, width: u32, height: u32) {
		let aspect = width as f32 / height as f32;
		let projection = Matrix4::new_perspective(aspect, f32::to_radians(45.0), 0.0, f32::MAX);

		let translation = Matrix4::from_euler_angles(self.rotation_y, self.rotation_x, 0.0)
			.mul(dvector![0.0, 0.0, self.distance, 0.0])
			.xyz();

		let view = Matrix4::look_at_rh(&Point3::from(translation), &Point3::new(0.0, 0.0, 0.0), &UP_VECTOR);
		let matrix = projection * view;

		queue.write_buffer(&self.buffer, 0, cast_slice(matrix.as_slice()))
	}

	pub fn handle_mouse_wheel(&mut self, event: MouseScrollDelta) {
		self.distance -= match event {
			LineDelta(_, y) => y,
			PixelDelta(PhysicalPosition { y, .. }) => y as f32,
		};
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
			}
			_ => {}
		}
	}
}
