use crate::camera::Camera;
use crate::data::{EdgeData, CELL_EDGE_MAP, CORNERS, EDGE_CORNER_MAP};
use bytemuck::{cast_slice, Pod, Zeroable};
use nalgebra::{Isometry3, Point3, Vector3};
use solarscape_shared::types::ChunkData;
use std::collections::HashMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
	include_wgsl, BlendState, Buffer, BufferUsages, ColorTargetState, ColorWrites, Device, FragmentState, FrontFace,
	MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass,
	RenderPipeline, RenderPipelineDescriptor, SurfaceConfiguration, VertexAttribute, VertexBufferLayout, VertexFormat,
	VertexState, VertexStepMode,
};

pub struct Sector {
	pub voxjects: Vec<Voxject>,

	chunk_pipeline: RenderPipeline,
}

impl Sector {
	#[must_use]
	pub fn new(config: &SurfaceConfiguration, camera: &Camera, device: &Device) -> Self {
		let chunk_shader = device.create_shader_module(include_wgsl!("chunk.wgsl"));

		let chunk_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some("sector.chunk_pipeline_layout"),
			bind_group_layouts: &[camera.bind_group_layout()],
			push_constant_ranges: &[],
		});

		let chunk_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
			label: Some("sector.chunk_pipeline"),
			layout: Some(&chunk_pipeline_layout),
			vertex: VertexState {
				module: &chunk_shader,
				entry_point: "vertex",
				buffers: &[
					VertexBufferLayout {
						array_stride: 12,
						step_mode: VertexStepMode::Vertex,
						attributes: &[VertexAttribute {
							offset: 0,
							shader_location: 0,
							format: VertexFormat::Float32x3,
						}],
					},
					VertexBufferLayout {
						array_stride: 16,
						step_mode: VertexStepMode::Instance,
						attributes: &[
							VertexAttribute { offset: 0, shader_location: 1, format: VertexFormat::Float32x3 },
							VertexAttribute { offset: 12, shader_location: 2, format: VertexFormat::Float32 },
						],
					},
				],
			},
			primitive: PrimitiveState {
				topology: PrimitiveTopology::TriangleList,
				strip_index_format: None,
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

		Self { voxjects: vec![], chunk_pipeline }
	}

	pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		render_pass.set_pipeline(&self.chunk_pipeline);

		self.voxjects
			.iter()
			.flat_map(|voxject| voxject.chunks.iter().flat_map(|level| level.values()))
			.for_each(|chunk| chunk.render(render_pass));
	}
}

pub struct Voxject {
	pub name: Box<str>,
	pub location: Isometry3<f32>,
	pub chunks: [HashMap<Vector3<i32>, Chunk>; 31],
}

pub struct Chunk {
	pub level: u8,
	pub coordinates: Vector3<i32>,

	pub data: ChunkData,

	pub mesh: Option<ChunkMesh>,
}

pub struct ChunkMesh {
	vertex_count: u32,
	vertex_buffer: Buffer,
	instance_buffer: Buffer,
}

impl Chunk {
	pub fn rebuild_mesh(&mut self, device: &Device) {
		let mut vertices = vec![];

		for x in 0..15 {
			for y in 0..15 {
				for z in 0..15 {
					#[allow(clippy::identity_op)]
					#[rustfmt::skip]
					let case_index = ((self.data[(x  ) << 8 | (y  ) << 4 | (z+1)] <= 4.0) as usize) << 0
					               | ((self.data[(x+1) << 8 | (y  ) << 4 | (z+1)] <= 4.0) as usize) << 1
					               | ((self.data[(x+1) << 8 | (y  ) << 4 | (z  )] <= 4.0) as usize) << 2
					               | ((self.data[(x  ) << 8 | (y  ) << 4 | (z  )] <= 4.0) as usize) << 3
					               | ((self.data[(x  ) << 8 | (y+1) << 4 | (z+1)] <= 4.0) as usize) << 4
					               | ((self.data[(x+1) << 8 | (y+1) << 4 | (z+1)] <= 4.0) as usize) << 5
					               | ((self.data[(x+1) << 8 | (y+1) << 4 | (z  )] <= 4.0) as usize) << 6
					               | ((self.data[(x  ) << 8 | (y+1) << 4 | (z  )] <= 4.0) as usize) << 7;

					let EdgeData { count, edge_indices } = CELL_EDGE_MAP[case_index];

					for edge_index in &edge_indices[0..count as usize] {
						let edge = EDGE_CORNER_MAP[*edge_index as usize];

						let a = interpolate(CORNERS[edge[0]], CORNERS[edge[1]]);
						let b = interpolate(CORNERS[edge[1]], CORNERS[edge[0]]);

						vertices.push(Point3::new(
							x as f32 + (a.x + b.x) / 2.0,
							y as f32 + (a.y + b.y) / 2.0,
							z as f32 + (a.z + b.z) / 2.0,
						));
					}
				}
			}
		}

		if vertices.is_empty() {
			self.mesh = None;
			return;
		}

		#[derive(Clone, Copy)]
		struct InstanceData {
			position: Vector3<f32>,
			scale: f32,
		}

		unsafe impl Zeroable for InstanceData {}
		unsafe impl Pod for InstanceData {}

		self.mesh = Some(ChunkMesh {
			vertex_count: vertices.len() as u32,
			vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh.vertex_buffer"),
				contents: cast_slice(&vertices),
				usage: BufferUsages::VERTEX,
			}),
			instance_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh.instance_buffer"),
				contents: cast_slice(&[InstanceData {
					position: self.coordinates.cast() * ((16u64 << self.level) + 1) as f32,
					scale: (self.level + 1) as f32,
				}]),
				usage: BufferUsages::VERTEX,
			}),
		});
	}

	pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		if self.level != 0 {
			return;
		}

		if let Some(ChunkMesh { vertex_count, vertex_buffer, instance_buffer }) = &self.mesh {
			render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
			render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
			render_pass.draw(0..*vertex_count, 0..1);
		}
	}
}

#[must_use]
fn interpolate(a: Point3<f32>, b: Point3<f32>) -> Point3<f32> {
	Point3::new(
		1.0_f32.mul_add(b.x - a.x, a.x),
		1.0_f32.mul_add(b.y - a.y, a.y),
		1.0_f32.mul_add(b.z - a.z, a.z),
	)
}
