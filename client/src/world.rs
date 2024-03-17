use crate::camera::Camera;
use bytemuck::cast_slice;
use log::info;
use nalgebra::{convert, Isometry3, Matrix4, Similarity3, Translation, Vector3, Vector4};
use std::{collections::HashMap, mem::size_of};
use wgpu::{
	include_wgsl, util::BufferInitDescriptor, util::DeviceExt, BlendState, Buffer, BufferAddress, BufferDescriptor,
	BufferUsages, ColorTargetState, ColorWrites, Device, FragmentState, FrontFace, IndexFormat, MultisampleState,
	PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline,
	RenderPipelineDescriptor, SurfaceConfiguration, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
	VertexStepMode,
};

#[rustfmt::skip]
pub const CHUNK_DEBUG_VERTICES: [f32; 24] = [
	0.0, 0.0, 0.0,
	0.0, 0.0, 1.0,
	0.0, 1.0, 0.0,
	0.0, 1.0, 1.0,
	1.0, 0.0, 0.0,
	1.0, 0.0, 1.0,
	1.0, 1.0, 0.0,
	1.0, 1.0, 1.0
];

#[rustfmt::skip]
pub const CHUNK_DEBUG_INDICES: [u16; 19] = [
	0, 1, 3, 2, 0, 4, 5, 7, 6, 4, 0xFFFF, 1, 5, 0xFFFF, 2, 6, 0xFFFF, 3, 7
];

pub struct World {
	pub voxjects: Vec<Voxject>,

	chunk_count: u32,
	pub changed: bool,

	chunk_pipeline: RenderPipeline,
	chunk_vertex_buffer: Buffer,
	chunk_index_buffer: Buffer,
	chunk_instance_buffer: Buffer,
}

impl World {
	#[must_use]
	pub fn new(config: &SurfaceConfiguration, camera: &Camera, device: &Device) -> Self {
		let chunk_shader = device.create_shader_module(include_wgsl!("chunk.wgsl"));

		let chunk_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some("world.chunk_pipeline_layout"),
			bind_group_layouts: &[camera.bind_group_layout()],
			push_constant_ranges: &[],
		});

		let chunk_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("world.chunk_pipeline"),
			layout: Some(&chunk_pipeline_layout),
			vertex: VertexState {
				module: &chunk_shader,
				entry_point: "vertex",
				buffers: &[
					VertexBufferLayout {
						array_stride: size_of::<Vector3<f32>>() as BufferAddress,
						step_mode: VertexStepMode::Vertex,
						attributes: &[VertexAttribute {
							offset: 0,
							shader_location: 0,
							format: VertexFormat::Float32x3,
						}],
					},
					VertexBufferLayout {
						array_stride: size_of::<Matrix4<f32>>() as BufferAddress,
						step_mode: VertexStepMode::Instance,
						attributes: &[
							VertexAttribute { offset: 0, shader_location: 1, format: VertexFormat::Float32x4 },
							VertexAttribute {
								offset: size_of::<Vector4<f32>>() as BufferAddress,
								shader_location: 2,
								format: VertexFormat::Float32x4,
							},
							VertexAttribute {
								offset: size_of::<Vector4<f32>>() as BufferAddress * 2,
								shader_location: 3,
								format: VertexFormat::Float32x4,
							},
							VertexAttribute {
								offset: size_of::<Vector4<f32>>() as BufferAddress * 3,
								shader_location: 4,
								format: VertexFormat::Float32x4,
							},
						],
					},
				],
			},
			primitive: PrimitiveState {
				topology: PrimitiveTopology::LineStrip,
				strip_index_format: Some(IndexFormat::Uint16),
				front_face: FrontFace::Ccw,
				cull_mode: None,
				unclipped_depth: false,
				polygon_mode: PolygonMode::Fill,
				conservative: false,
			},
			depth_stencil: None,
			multisample: MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
			fragment: Some(FragmentState {
				module: &chunk_shader,
				entry_point: "fragment",
				targets: &[Some(ColorTargetState {
					format: config.format,
					blend: Some(BlendState::REPLACE),
					write_mask: ColorWrites::ALL,
				})],
			}),
			multiview: None,
		});

		let chunk_vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
			label: Some("world.chunk_vertex_buffer"),
			contents: cast_slice(&CHUNK_DEBUG_VERTICES),
			usage: BufferUsages::VERTEX,
		});

		let chunk_index_buffer = device.create_buffer_init(&BufferInitDescriptor {
			label: Some("world.chunk_index_buffer"),
			contents: cast_slice(&CHUNK_DEBUG_INDICES),
			usage: BufferUsages::INDEX,
		});

		let chunk_instance_buffer = device.create_buffer(&BufferDescriptor {
			label: Some("world.chunk_instance_buffer"),
			size: 0,
			usage: BufferUsages::VERTEX,
			mapped_at_creation: false,
		});

		Self {
			voxjects: Vec::new(),

			chunk_count: 0,
			changed: false,

			chunk_pipeline,
			chunk_vertex_buffer,
			chunk_index_buffer,
			chunk_instance_buffer,
		}
	}

	pub fn render<'a>(&'a mut self, device: &Device, render_pass: &mut RenderPass<'a>) {
		if self.changed {
			// TODO: costly, dumb, and jank, good thing its temporary!
			let chunk_debug_instances = self
				.voxjects
				.iter()
				.flat_map(|voxject| {
					voxject.chunks.iter().enumerate().flat_map(move |(level, chunks)| {
						chunks.keys().map(move |grid_position| {
							let position: Vector3<f32> =
								convert(grid_position.map(|value| value as i64 * (16 << level)));
							Similarity3::from_parts(
								Translation::from(position),
								voxject.location.rotation,
								(16u64 << level) as f32,
							)
							.to_homogeneous()
						})
					})
				})
				.collect::<Vec<_>>();

			self.chunk_count = chunk_debug_instances.len() as u32;
			info!("Updated chunk_debug_buffer with {} chunks", self.chunk_count);

			self.chunk_instance_buffer = device.create_buffer_init(&BufferInitDescriptor {
				label: Some("world.chunk_instance_buffer"),
				contents: cast_slice(&chunk_debug_instances),
				usage: BufferUsages::VERTEX,
			});

			self.changed = false;
		}

		render_pass.set_pipeline(&self.chunk_pipeline);
		render_pass.set_vertex_buffer(0, self.chunk_vertex_buffer.slice(..));
		render_pass.set_vertex_buffer(1, self.chunk_instance_buffer.slice(..));
		render_pass.set_index_buffer(self.chunk_index_buffer.slice(..), IndexFormat::Uint16);
		render_pass.draw_indexed(0..CHUNK_DEBUG_INDICES.len() as u32, 0, 0..self.chunk_count);
	}
}

pub struct Voxject {
	pub name: String,
	pub location: Isometry3<f32>,
	pub chunks: [HashMap<Vector3<i32>, Chunk>; 31],
}

pub struct Chunk;
