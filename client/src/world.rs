use crate::{camera::Camera, chunk::Chunk};
use nalgebra::{Isometry3, Vector3};
use std::collections::HashMap;
use wgpu::{
	include_wgsl, BlendState, ColorTargetState, ColorWrites, Device, FragmentState, FrontFace, IndexFormat,
	MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass,
	RenderPipeline, RenderPipelineDescriptor, SurfaceConfiguration, VertexAttribute, VertexBufferLayout, VertexFormat,
	VertexState, VertexStepMode,
};

pub struct World {
	pub voxjects: Vec<Voxject>,

	chunk_pipeline: RenderPipeline,
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
	pub name: String,
	pub location: Isometry3<f32>,
	pub chunks: [HashMap<Vector3<i32>, Chunk>; 31],
}
