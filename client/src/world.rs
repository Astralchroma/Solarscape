use crate::{camera::Camera, data::EdgeData, data::CELL_EDGE_MAP, data::CORNERS, data::EDGE_CORNER_MAP};
use bytemuck::{cast_slice, Pod, Zeroable};
use nalgebra::{vector, Isometry3, Point3, Vector3, Vector4};
use solarscape_shared::types::ChunkData;
use std::collections::{HashMap, HashSet};
use wgpu::{
	include_wgsl, util::BufferInitDescriptor, util::DeviceExt, BlendState, Buffer, BufferUsages, ColorTargetState,
	ColorWrites, Device, FragmentState, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode,
	PrimitiveState, PrimitiveTopology, RenderPass, RenderPipeline, RenderPipelineDescriptor, SurfaceConfiguration,
	VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
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

	pub dependent_chunks: HashMap<Vector4<i32>, HashSet<Vector4<i32>>>,

	pub chunks: [HashMap<Vector3<i32>, Chunk>; 31],
}

// This code is admittedly absolutely fucking terrible, it just needs to work
impl Voxject {
	pub fn add_chunk(&mut self, device: &Device, chunk: Chunk) {
		let coordinates = chunk.coordinates;
		let level = chunk.level as usize;

		self.chunks[level].insert(coordinates, chunk);

		// Rebuild any chunks that need this chunk
		{
			let coordinates = vector![coordinates.x, coordinates.y, coordinates.z, level as i32];
			if let Some(dependent_chunks) = self.dependent_chunks.get(&coordinates).cloned() {
				for dependent_chunk in dependent_chunks {
					self.try_build_chunk(device, dependent_chunk.xyz(), level);
				}
			}
		}

		self.try_build_chunk(device, coordinates, level);
	}

	pub fn try_build_chunk(
		&mut self,
		device: &Device,
		current_chunk_coordinates: Vector3<i32>,
		current_chunk_level: usize,
	) {
		let chunk_coordinates = [
			current_chunk_coordinates + Vector3::new(0, 0, 0),
			current_chunk_coordinates + Vector3::new(0, 0, 1),
			current_chunk_coordinates + Vector3::new(0, 1, 0),
			current_chunk_coordinates + Vector3::new(0, 1, 1),
			current_chunk_coordinates + Vector3::new(1, 0, 0),
			current_chunk_coordinates + Vector3::new(1, 0, 1),
			current_chunk_coordinates + Vector3::new(1, 1, 0),
			current_chunk_coordinates + Vector3::new(1, 1, 1),
		];

		let chunks = chunk_coordinates.map(|coordinates| self.chunks[current_chunk_level].get(&coordinates));
		let uplevel = current_chunk_level != 30;
		let mut upleveled_chunk_coordinates = Default::default();
		let mut upleveled_chunks = Default::default();
		if uplevel {
			upleveled_chunk_coordinates = chunk_coordinates.map(|coordinates| coordinates / 2);
			upleveled_chunks =
				upleveled_chunk_coordinates.map(|coordinates| self.chunks[current_chunk_level + 1].get(&coordinates));
		}

		let mut data = [0; 17 * 17 * 17];
		let mut need_upleveled_chunks = false;

		'x: for x in 0..17 {
			for y in 0..17 {
				for z in 0..17 {
					// messy but probably fast?
					let chunk_index = ((x & 0x10) >> 2) | ((y & 0x10) >> 3) | ((z & 0x10) >> 4);

					// The actual chunk we need is loaded, yay! This is the easy path.
					if let Some(chunk) = chunks[chunk_index] {
						// Data expands a little bit further than chunk data, so we can't just copy the chunk data array
						// instead we have to map it to the data
						data[(x * 289) + (y * 17) + z] = chunk.data[(x & 0x0F) << 8 | (y & 0x0F) << 4 | z & 0x0F];
						continue;
					}

					if uplevel {
						// Now what if that chunk isn't loaded and we need to get the data from higher level chunks...?
						//
						// Upleveling coordinates is essentially `coordinates / 2`, however because these are relative
						// coordinates and not global ones, we need to offset them based on the center chunk's position
						// in the upleveled chunk.
						let u_x = ((current_chunk_coordinates.x as usize & 1) * 8) + (x >> 1);
						let u_y = ((current_chunk_coordinates.y as usize & 1) * 8) + (y >> 1);
						let u_z = ((current_chunk_coordinates.z as usize & 1) * 8) + (z >> 1);

						// Now we do the same thing we would do normally, except operating on upleveled chunks
						let upleveled_chunk_index = ((u_x & 0x10) >> 2) | ((u_y & 0x10) >> 3) | ((u_z & 0x10) >> 4);

						if let Some(chunk) = upleveled_chunks[upleveled_chunk_index] {
							data[(x * 289) + (y * 17) + z] =
								chunk.data[(u_x & 0x0F) << 8 | (u_y & 0x0F) << 4 | u_z & 0x0F];
							continue;
						}

						// Missing upleveled chunks too, so we can't build this chunk at all
						// Mark this to be rebuild it any upleveled chunks get synced, and then break
						need_upleveled_chunks = true;
					}

					break 'x;
				}
			}
		}

		let current_coordinates = vector![
			current_chunk_coordinates.x,
			current_chunk_coordinates.y,
			current_chunk_coordinates.z,
			current_chunk_level as i32
		];
		let upleveled_current_coordinates = vector![
			current_chunk_coordinates.x >> 1,
			current_chunk_coordinates.y >> 1,
			current_chunk_coordinates.z >> 1,
			current_chunk_level as i32 + 1
		];

		// Make sure we are rebuilt if any chunks we depend on are changed
		for coordinates in chunk_coordinates {
			let coordinates = vector![coordinates.x, coordinates.y, coordinates.z, current_chunk_level as i32];
			match self.dependent_chunks.get_mut(&coordinates) {
				None => {
					self.dependent_chunks
						.insert(coordinates, HashSet::from([current_coordinates]));
				}
				Some(dependent_chunks) => {
					dependent_chunks.insert(current_coordinates);
				}
			}
		}

		if uplevel {
			// Now either add or remove our dependency on upleveled chunks
			for coordinates in upleveled_chunk_coordinates {
				let coordinates = vector![
					coordinates.x,
					coordinates.y,
					coordinates.z,
					current_chunk_level as i32 + 1
				];
				match self.dependent_chunks.get_mut(&coordinates) {
					None if need_upleveled_chunks => {
						self.dependent_chunks
							.insert(coordinates, HashSet::from([upleveled_current_coordinates]));
					}
					Some(dependent_chunks) => {
						match need_upleveled_chunks {
							true => dependent_chunks.insert(upleveled_current_coordinates),
							false => dependent_chunks.remove(&upleveled_current_coordinates),
						};

						if dependent_chunks.is_empty() {
							self.dependent_chunks.remove(&coordinates);
						}
					}
					_ => {}
				}
			}
		}

		if let Some(chunk) = self.chunks[current_chunk_level].get_mut(&current_chunk_coordinates) {
			// Not enough data to build chunk
			if need_upleveled_chunks {
				chunk.mesh = None;
				return;
			}

			// Now we can build the chunk mesh
			chunk.rebuild_mesh(device, data)
		}
	}
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
	pub fn rebuild_mesh(&mut self, device: &Device, data: [u8; 17 * 17 * 17]) {
		let mut vertices = vec![];

		for x in 0..16 {
			for y in 0..16 {
				for z in 0..16 {
					#[allow(clippy::identity_op)]
					#[rustfmt::skip]
					let case_index = ((data[((x  ) * 289) + ((y  ) * 17) + (z+1)] <= 5) as usize) << 0
					               | ((data[((x+1) * 289) + ((y  ) * 17) + (z+1)] <= 5) as usize) << 1
					               | ((data[((x+1) * 289) + ((y  ) * 17) + (z  )] <= 5) as usize) << 2
					               | ((data[((x  ) * 289) + ((y  ) * 17) + (z  )] <= 5) as usize) << 3
					               | ((data[((x  ) * 289) + ((y+1) * 17) + (z+1)] <= 5) as usize) << 4
					               | ((data[((x+1) * 289) + ((y+1) * 17) + (z+1)] <= 5) as usize) << 5
					               | ((data[((x+1) * 289) + ((y+1) * 17) + (z  )] <= 5) as usize) << 6
					               | ((data[((x  ) * 289) + ((y+1) * 17) + (z  )] <= 5) as usize) << 7;

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
					position: self.coordinates.cast() * (16u64 << self.level) as f32,
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
