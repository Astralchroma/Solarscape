use crate::types::Radians;
use bytemuck::cast_slice;
use nalgebra::{IsometryMatrix3, Matrix4, Perspective3};
use std::mem::size_of;
use wgpu::{
	BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
	BindingType, Buffer, BufferAddress, BufferBindingType::Uniform, BufferDescriptor, BufferUsages, Device, Queue,
	RenderPass, ShaderStages,
};

pub struct Camera {
	perspective: Perspective3<f32>,
	view: IsometryMatrix3<f32>,
	changed: bool,

	bind_group_layout: BindGroupLayout,
	buffer: Buffer,
	bind_group: BindGroup,
}

impl Camera {
	#[must_use]
	pub fn new(aspect: f32, fov_y: impl Into<Radians>, device: &Device) -> Self {
		let perspective = Perspective3::new(aspect, *fov_y.into(), 0.0, f32::MAX);
		let view = IsometryMatrix3::identity();

		let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
			label: Some("camera.bind_group_layout"),
			entries: &[BindGroupLayoutEntry {
				binding: 0,
				visibility: ShaderStages::VERTEX,
				ty: BindingType::Buffer {
					ty: Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
		});

		let buffer = device.create_buffer(&BufferDescriptor {
			label: Some("camera.buffer"),
			size: size_of::<Matrix4<f32>>() as BufferAddress,
			usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
			mapped_at_creation: false,
		});

		let bind_group = device.create_bind_group(&BindGroupDescriptor {
			label: Some("camera.bind_group"),
			layout: &bind_group_layout,
			entries: &[BindGroupEntry {
				binding: 0,
				resource: buffer.as_entire_binding(),
			}],
		});

		Self {
			perspective,
			view,
			changed: true,
			bind_group_layout,
			buffer,
			bind_group,
		}
	}

	#[must_use]
	pub const fn bind_group_layout(&self) -> &BindGroupLayout {
		&self.bind_group_layout
	}

	pub fn set_aspect(&mut self, aspect: f32) {
		self.perspective.set_aspect(aspect);
		self.changed = true;
	}

	pub fn set_view(&mut self, view: IsometryMatrix3<f32>) {
		self.view = view;
		self.changed = true;
	}

	pub fn use_camera<'a>(&'a mut self, queue: &Queue, render_pass: &mut RenderPass<'a>) {
		if self.changed {
			let camera = self.perspective.to_homogeneous() * self.view.to_homogeneous();
			queue.write_buffer(&self.buffer, 0, cast_slice(&[camera]));
			self.changed = false
		}
		render_pass.set_bind_group(0, &self.bind_group, &[]);
	}
}
