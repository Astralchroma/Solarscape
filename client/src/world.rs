use crate::{camera::Camera, data::EdgeData, data::CELL_EDGE_MAP, data::CORNERS, data::EDGE_CORNER_MAP};
use bytemuck::{cast_slice, Pod, Zeroable};
use nalgebra::{Isometry3, Point3, Vector3};
use solarscape_shared::types::{ChunkData, GridCoordinates};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use wgpu::{
	include_wgsl, util::BufferInitDescriptor, util::DeviceExt, BlendState, Buffer, BufferUsages, ColorTargetState,
	ColorWrites, CompareFunction::GreaterEqual, DepthStencilState, Device, Face::Back, FragmentState, FrontFace,
	MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPass,
	RenderPipeline, RenderPipelineDescriptor, SurfaceConfiguration, TextureFormat::Depth32Float, VertexAttribute,
	VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
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
				cull_mode: Some(Back),
				unclipped_depth: false,
				polygon_mode: PolygonMode::Fill,
				conservative: false,
			},
			depth_stencil: Some(DepthStencilState {
				format: Depth32Float,
				depth_write_enabled: true,
				depth_compare: GreaterEqual,
				stencil: Default::default(),
				bias: Default::default(),
			}),
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

	pub dependent_chunks: HashMap<GridCoordinates, HashSet<GridCoordinates>>,

	pub chunks: [HashMap<Vector3<i32>, Chunk>; 31],
}

// This code is admittedly absolutely fucking terrible, it just needs to work
impl Voxject {
	pub fn add_chunk(&mut self, device: &Device, chunk: Chunk) {
		let grid_coordinates = chunk.grid_coordinates;
		self.chunks[grid_coordinates.level as usize].insert(grid_coordinates.coordinates, chunk);

		// Rebuild any chunks that need this chunk
		{
			if let Some(dependent_chunks) = self.dependent_chunks.get(&grid_coordinates).cloned() {
				for dependent_chunk in dependent_chunks {
					self.try_build_chunk(device, dependent_chunk);
				}
			}
		}

		self.try_build_chunk(device, grid_coordinates);
	}

	pub fn try_build_chunk(&mut self, device: &Device, grid_coordinates: GridCoordinates) {
		let dependency_grid_coordinates = [
			grid_coordinates.coordinates + Vector3::new(0, 0, 0),
			grid_coordinates.coordinates + Vector3::new(0, 0, 1),
			grid_coordinates.coordinates + Vector3::new(0, 1, 0),
			grid_coordinates.coordinates + Vector3::new(0, 1, 1),
			grid_coordinates.coordinates + Vector3::new(1, 0, 0),
			grid_coordinates.coordinates + Vector3::new(1, 0, 1),
			grid_coordinates.coordinates + Vector3::new(1, 1, 0),
			grid_coordinates.coordinates + Vector3::new(1, 1, 1),
		];

		let dependency_chunks = dependency_grid_coordinates
			.map(|coordinates| self.chunks[grid_coordinates.level as usize].get(&coordinates));
		let should_uplevel = grid_coordinates.level != 30;
		let mut upleveled_dependency_grid_coordinates = Default::default();
		let mut upleveled_dependency_chunks = Default::default();
		if should_uplevel {
			upleveled_dependency_grid_coordinates = dependency_grid_coordinates.map(|coordinates| coordinates / 2);
			upleveled_dependency_chunks = upleveled_dependency_grid_coordinates
				.map(|coordinates| self.chunks[grid_coordinates.level as usize + 1].get(&coordinates));
		}

		let mut data = [0; 17 * 17 * 17];
		let mut need_upleveled_chunks = false;

		'x: for x in 0..17 {
			for y in 0..17 {
				for z in 0..17 {
					// messy but probably fast?
					let chunk_index = ((x & 0x10) >> 2) | ((y & 0x10) >> 3) | ((z & 0x10) >> 4);

					// The actual chunk we need is loaded, yay! This is the easy path.
					if let Some(chunk) = dependency_chunks[chunk_index] {
						// Data expands a little bit further than chunk data, so we can't just copy the chunk data array
						// instead we have to map it to the data
						data[(x * 289) + (y * 17) + z] = chunk.densities[(x & 0x0F) << 8 | (y & 0x0F) << 4 | z & 0x0F];
						continue;
					}

					if should_uplevel {
						// Now what if that chunk isn't loaded and we need to get the data from higher level chunks...?
						//
						// Upleveling coordinates is essentially `coordinates / 2`, however because these are relative
						// coordinates and not global ones, we need to offset them based on the center chunk's position
						// in the upleveled chunk.
						let u_x = ((grid_coordinates.coordinates.x as usize & 1) * 8) + (x >> 1);
						let u_y = ((grid_coordinates.coordinates.y as usize & 1) * 8) + (y >> 1);
						let u_z = ((grid_coordinates.coordinates.z as usize & 1) * 8) + (z >> 1);

						// Now we do the same thing we would do normally, except operating on upleveled chunks
						let upleveled_chunk_index = ((u_x & 0x10) >> 2) | ((u_y & 0x10) >> 3) | ((u_z & 0x10) >> 4);

						if let Some(chunk) = upleveled_dependency_chunks[upleveled_chunk_index] {
							data[(x * 289) + (y * 17) + z] =
								chunk.densities[(u_x & 0x0F) << 8 | (u_y & 0x0F) << 4 | u_z & 0x0F];
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

		let upleveled_grid_coordinates = grid_coordinates.uplevel();

		// Make sure we are rebuilt if any chunks we depend on are changed
		for level_coordinates in dependency_grid_coordinates {
			let dependency_grid_coordinates =
				GridCoordinates { coordinates: level_coordinates, level: grid_coordinates.level };
			match self.dependent_chunks.get_mut(&dependency_grid_coordinates) {
				None => {
					self.dependent_chunks
						.insert(dependency_grid_coordinates, HashSet::from([grid_coordinates]));
				}
				Some(dependent_chunks) => {
					dependent_chunks.insert(grid_coordinates);
				}
			}
		}

		if should_uplevel {
			// Now either add or remove our dependency on upleveled chunks
			for level_coordinates in upleveled_dependency_grid_coordinates {
				let upleveled_dependency_grid_coordinates =
					GridCoordinates { coordinates: level_coordinates, level: upleveled_grid_coordinates.level };
				match self.dependent_chunks.get_mut(&upleveled_dependency_grid_coordinates) {
					None if need_upleveled_chunks => {
						self.dependent_chunks.insert(
							upleveled_dependency_grid_coordinates,
							HashSet::from([upleveled_grid_coordinates]),
						);
					}
					Some(dependent_chunks) => {
						match need_upleveled_chunks {
							true => dependent_chunks.insert(upleveled_grid_coordinates),
							false => dependent_chunks.remove(&upleveled_grid_coordinates),
						};

						if dependent_chunks.is_empty() {
							self.dependent_chunks.remove(&upleveled_dependency_grid_coordinates);
						}
					}
					_ => {}
				}
			}
		}

		if let Some(chunk) = self.chunks[grid_coordinates.level as usize].get_mut(&grid_coordinates.coordinates) {
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
	pub data: ChunkData,
	pub mesh: Option<ChunkMesh>,
}

impl Deref for Chunk {
	type Target = ChunkData;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl DerefMut for Chunk {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
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
					let case_index = ((data[((x  ) * 289) + ((y  ) * 17) + (z+1)] != 0) as usize) << 0
					               | ((data[((x+1) * 289) + ((y  ) * 17) + (z+1)] != 0) as usize) << 1
					               | ((data[((x+1) * 289) + ((y  ) * 17) + (z  )] != 0) as usize) << 2
					               | ((data[((x  ) * 289) + ((y  ) * 17) + (z  )] != 0) as usize) << 3
					               | ((data[((x  ) * 289) + ((y+1) * 17) + (z+1)] != 0) as usize) << 4
					               | ((data[((x+1) * 289) + ((y+1) * 17) + (z+1)] != 0) as usize) << 5
					               | ((data[((x+1) * 289) + ((y+1) * 17) + (z  )] != 0) as usize) << 6
					               | ((data[((x  ) * 289) + ((y+1) * 17) + (z  )] != 0) as usize) << 7;

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

		#[allow(unused)]
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
					position: self.grid_coordinates.coordinates.cast() * (16u64 << self.grid_coordinates.level) as f32,
					scale: (self.grid_coordinates.level + 1) as f32,
				}]),
				usage: BufferUsages::VERTEX,
			}),
		});
	}

	pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		if self.grid_coordinates.level != 0 {
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
