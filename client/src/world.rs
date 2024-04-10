use crate::{camera::Camera, data::EdgeData, data::CELL_EDGE_MAP, data::CORNERS, data::EDGE_CORNER_MAP};
use bytemuck::{cast_slice, Pod, Zeroable};
use image::GenericImageView;
use nalgebra::{vector, Isometry3, Vector2, Vector3};
use solarscape_shared::types::{ChunkData, GridCoordinates, Material};
use std::{collections::HashMap, collections::HashSet, ops::Deref, ops::DerefMut};
use wgpu::{
	include_wgsl, util::BufferInitDescriptor, util::DeviceExt, BindGroup, BindGroupDescriptor, BindGroupEntry,
	BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferUsages,
	ColorTargetState, ColorWrites, CompareFunction::GreaterEqual, DepthStencilState, Device, Extent3d, Face::Back,
	FragmentState, FrontFace, ImageCopyTexture, ImageDataLayout, MultisampleState, Origin3d, PipelineLayoutDescriptor,
	PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor,
	SamplerBindingType::NonFiltering, SamplerDescriptor, ShaderStages, SurfaceConfiguration, TextureAspect::All,
	TextureDescriptor, TextureDimension, TextureFormat::Depth32Float, TextureFormat::Rgba8UnormSrgb, TextureSampleType,
	TextureUsages, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout,
	VertexFormat::Float32, VertexFormat::Float32x3, VertexFormat::Uint8x2, VertexState, VertexStepMode::Instance,
	VertexStepMode::Vertex,
};

pub struct Sector {
	pub voxjects: Vec<Voxject>,

	terrain_textures_bind_group: BindGroup,
	chunk_pipeline: RenderPipeline,
}

impl Sector {
	#[must_use]
	pub fn new(config: &SurfaceConfiguration, camera: &Camera, device: &Device, queue: &Queue) -> Self {
		let terrain_textures_image = image::load_from_memory(include_bytes!("terrain_textures.png"))
			.expect("terrain_textures.png must be valid");
		let terrain_textures_rgba8 = terrain_textures_image.to_rgba8();
		let (terrain_textures_width, terrain_textures_height) = terrain_textures_image.dimensions();
		let terrain_textures_size =
			Extent3d { width: terrain_textures_width, height: terrain_textures_height, depth_or_array_layers: 1 };

		let terrain_textures = device.create_texture(&TextureDescriptor {
			label: Some("sector.terrain_textures"),
			size: terrain_textures_size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: TextureDimension::D2,
			format: Rgba8UnormSrgb,
			usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
			view_formats: &[],
		});

		queue.write_texture(
			ImageCopyTexture { texture: &terrain_textures, mip_level: 0, origin: Origin3d::ZERO, aspect: All },
			&terrain_textures_rgba8,
			ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(4 * terrain_textures_width),
				rows_per_image: Some(terrain_textures_height),
			},
			terrain_textures_size,
		);

		let terrain_textures_view = terrain_textures.create_view(&TextureViewDescriptor::default());
		let terrain_textures_sampler = device.create_sampler(&SamplerDescriptor::default());

		let terrain_textures_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
			label: Some("sector.terrain_textures_bind_group_layout"),
			entries: &[
				BindGroupLayoutEntry {
					binding: 0,
					visibility: ShaderStages::FRAGMENT,
					ty: BindingType::Texture {
						sample_type: TextureSampleType::Float { filterable: false },
						view_dimension: TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				BindGroupLayoutEntry {
					binding: 1,
					visibility: ShaderStages::FRAGMENT,
					ty: BindingType::Sampler(NonFiltering),
					count: None,
				},
			],
		});

		let terrain_textures_bind_group = device.create_bind_group(&BindGroupDescriptor {
			label: Some("sector.terrain_textures_bind_group"),
			layout: &terrain_textures_bind_group_layout,
			entries: &[
				BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&terrain_textures_view) },
				BindGroupEntry { binding: 1, resource: BindingResource::Sampler(&terrain_textures_sampler) },
			],
		});

		let chunk_shader = device.create_shader_module(include_wgsl!("chunk.wgsl"));

		let chunk_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some("sector.chunk_pipeline_layout"),
			bind_group_layouts: &[camera.bind_group_layout(), &terrain_textures_bind_group_layout],
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
						array_stride: 32,
						step_mode: Vertex,
						attributes: &[
							VertexAttribute { offset: 0, shader_location: 0, format: Float32x3 },
							VertexAttribute { offset: 12, shader_location: 1, format: Float32x3 },
							VertexAttribute { offset: 24, shader_location: 2, format: Uint8x2 },
							VertexAttribute { offset: 26, shader_location: 3, format: Uint8x2 },
							VertexAttribute { offset: 28, shader_location: 4, format: Float32 },
						],
					},
					VertexBufferLayout {
						array_stride: 16,
						step_mode: Instance,
						attributes: &[
							VertexAttribute { offset: 0, shader_location: 5, format: Float32x3 },
							VertexAttribute { offset: 12, shader_location: 6, format: Float32 },
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

		Self { voxjects: vec![], chunk_pipeline, terrain_textures_bind_group }
	}

	pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
		render_pass.set_pipeline(&self.chunk_pipeline);
		render_pass.set_bind_group(1, &self.terrain_textures_bind_group, &[]);

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

		let mut densities = [0.0; 17 * 17 * 17];
		let mut materials = [Material::Nothing; 17 * 17 * 17];
		let mut need_upleveled_chunks = false;

		'x: for x in 0..17 {
			for y in 0..17 {
				for z in 0..17 {
					// messy but probably fast?
					let chunk_index = ((x & 0x10) >> 2) | ((y & 0x10) >> 3) | ((z & 0x10) >> 4);
					let cell_index = (x * 289) + (y * 17) + z;

					// The actual chunk we need is loaded, yay! This is the easy path.
					if let Some(chunk) = dependency_chunks[chunk_index] {
						// Data expands a little bit further than chunk data, so we can't just copy the chunk data array
						// instead we have to map it to the
						let chunk_cell_index = (x & 0x0F) << 8 | (y & 0x0F) << 4 | z & 0x0F;
						densities[cell_index] = chunk.densities[chunk_cell_index];
						materials[cell_index] = chunk.materials[chunk_cell_index];
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
							let u_chunk_cell_index = (u_x & 0x0F) << 8 | (u_y & 0x0F) << 4 | u_z & 0x0F;
							densities[cell_index] = chunk.densities[u_chunk_cell_index];
							materials[cell_index] = chunk.materials[u_chunk_cell_index];
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
			chunk.rebuild_mesh(device, densities, materials);
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
	pub fn rebuild_mesh(
		&mut self,
		device: &Device,
		densities: [f32; 17 * 17 * 17],
		materials: [Material; 17 * 17 * 17],
	) {
		let mut vertex_data = vec![];

		#[allow(unused)]
		#[derive(Clone, Copy)]
		#[repr(packed)]
		struct VertexData {
			position: Vector3<f32>,
			normal: Vector3<f32>,
			material_a: Vector2<u8>,
			material_b: Vector2<u8>,
			weight: f32,
		}

		unsafe impl Zeroable for VertexData {}
		unsafe impl Pod for VertexData {}

		for x in 0..16 {
			for y in 0..16 {
				for z in 0..16 {
					let indexes = [
						(x, y, z + 1),
						(x + 1, y, z + 1),
						(x + 1, y, z),
						(x, y, z),
						(x, y + 1, z + 1),
						(x + 1, y + 1, z + 1),
						(x + 1, y + 1, z),
						(x, y + 1, z),
					]
					.map(|(x, y, z)| (x * 289) + (y * 17) + z);

					let densities = indexes.map(|index| densities[index]);
					let materials = indexes.map(|index| materials[index]);

					#[allow(clippy::identity_op)]
					#[rustfmt::skip]
					let case_index = (!matches!(materials[0], Material::Nothing) as usize) << 0
					               | (!matches!(materials[1], Material::Nothing) as usize) << 1
					               | (!matches!(materials[2], Material::Nothing) as usize) << 2
					               | (!matches!(materials[3], Material::Nothing) as usize) << 3
					               | (!matches!(materials[4], Material::Nothing) as usize) << 4
					               | (!matches!(materials[5], Material::Nothing) as usize) << 5
					               | (!matches!(materials[6], Material::Nothing) as usize) << 6
					               | (!matches!(materials[7], Material::Nothing) as usize) << 7;

					let EdgeData { count, edge_indices } = CELL_EDGE_MAP[case_index];

					for edge_indices in edge_indices.chunks(3).take(count as usize) {
						let mut vertices = edge_indices
							.iter()
							.map(|edge_index| {
								let (a_index, b_index) = EDGE_CORNER_MAP[*edge_index as usize];

								let a_density = densities[a_index];
								let b_density = densities[b_index];

								let weight = if (b_density - a_density).abs() > 1.0 {
									(0.0 - a_density) / (b_density - a_density)
								} else {
									0.5
								};

								let a = CORNERS[a_index];
								let b = CORNERS[b_index];

								let vertex = a + weight * (b - a);

								let a_material = if matches!(materials[a_index], Material::Nothing) {
									materials[b_index]
								} else {
									materials[a_index]
								};
								let b_material = if matches!(materials[b_index], Material::Nothing) {
									materials[a_index]
								} else {
									materials[b_index]
								};

								VertexData {
									position: vector![x as f32, y as f32, z as f32] + vertex,
									normal: Vector3::default(),
									material_a: vector![(a_material as u8 & 0xC) >> 2, a_material as u8 & 0x2],
									material_b: vector![(b_material as u8 & 0xC) >> 2, b_material as u8 & 0x2],
									weight,
								}
							})
							.collect::<Vec<_>>();

						let normal = (vertices[1].position - vertices[0].position)
							.cross(&(vertices[2].position - vertices[0].position))
							.normalize();

						vertices[0].normal = normal;
						vertices[1].normal = normal;
						vertices[2].normal = normal;

						vertex_data.extend_from_slice(&vertices);
					}
				}
			}
		}

		if vertex_data.is_empty() {
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
			vertex_count: vertex_data.len() as u32,
			vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh.vertex_buffer"),
				contents: cast_slice(&vertex_data),
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
