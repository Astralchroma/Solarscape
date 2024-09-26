use crate::{connection::Connection, player::Local, player::Player};
use bytemuck::{cast_slice, Pod, Zeroable};
use nalgebra::{vector, Isometry3, Vector2, Vector3};
use solarscape_shared::triangulation_table::{EdgeData, CELL_EDGE_MAP, CORNERS, EDGE_CORNER_MAP};
use solarscape_shared::types::{ChunkCoordinates, Material, VoxjectId};
use std::{collections::HashMap, collections::HashSet};
use wgpu::{util::BufferInitDescriptor, util::DeviceExt, Buffer, BufferUsages, Device};

pub struct Sector {
	pub player: Player<Local>,

	pub voxjects: HashMap<VoxjectId, Voxject>,
}

impl Sector {
	pub fn new(connection: Connection) -> Self {
		let player = Player::new(connection);

		Self {
			player,

			voxjects: HashMap::new(),
		}
	}
}

pub struct Voxject {
	pub id: VoxjectId,
	pub name: Box<str>,
	pub location: Isometry3<f32>,

	pub dependent_chunks: HashMap<ChunkCoordinates, HashSet<ChunkCoordinates>>,

	pub chunks: [HashMap<Vector3<i32>, Chunk>; 31],
}

// This code is admittedly absolutely fucking terrible, it just needs to work
impl Voxject {
	pub fn add_chunk(&mut self, device: &Device, chunk: Chunk) {
		let coordinates = chunk.coordinates;
		self.chunks[*coordinates.level as usize].insert(**coordinates, chunk);

		// Rebuild any chunks that need this chunk
		{
			if let Some(dependent_chunks) = self.dependent_chunks.get(&coordinates).cloned() {
				for dependent_chunk in dependent_chunks {
					self.try_build_chunk(device, dependent_chunk);
				}
			}
		}

		self.try_build_chunk(device, coordinates);
	}

	pub fn remove_chunk(&mut self, device: &Device, coordinates: ChunkCoordinates) {
		self.chunks[*coordinates.level as usize].remove(&**coordinates);

		if let Some(dependent_chunks) = self.dependent_chunks.get(&coordinates).cloned() {
			for dependent_chunk in dependent_chunks {
				self.try_build_chunk(device, dependent_chunk);
			}
		}
	}

	pub fn try_build_chunk(&mut self, device: &Device, grid_coordinates: ChunkCoordinates) {
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
			.map(|coordinates| self.chunks[*grid_coordinates.level as usize].get(&coordinates));
		let should_uplevel = *grid_coordinates.level != 30;
		let mut upleveled_dependency_grid_coordinates = Default::default();
		let mut upleveled_dependency_chunks = Default::default();
		if should_uplevel {
			upleveled_dependency_grid_coordinates = dependency_grid_coordinates.map(|coordinates| coordinates / 2);
			upleveled_dependency_chunks = upleveled_dependency_grid_coordinates
				.map(|coordinates| self.chunks[*grid_coordinates.level as usize + 1].get(&coordinates));
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

		let upleveled_grid_coordinates = grid_coordinates.upleveled();

		// Make sure we are rebuilt if any chunks we depend on are changed
		for level_coordinates in dependency_grid_coordinates {
			let dependency_grid_coordinates = ChunkCoordinates::new(self.id, level_coordinates, grid_coordinates.level);
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
					ChunkCoordinates::new(self.id, level_coordinates, upleveled_grid_coordinates.level);
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

		if let Some(chunk) = self.chunks[*grid_coordinates.level as usize].get_mut(&grid_coordinates.coordinates) {
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

#[non_exhaustive]
pub struct Chunk {
	pub coordinates: ChunkCoordinates,
	pub materials: Box<[Material; 4096]>,
	pub densities: Box<[f32; 4096]>,
	pub mesh: Option<ChunkMesh>,
}

pub struct ChunkMesh {
	pub vertex_count: u32,
	pub vertex_buffer: Buffer,
	pub instance_buffer: Buffer,
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

								let weight = if a_density == b_density {
									0.5
								} else {
									(0.0 - a_density) / (b_density - a_density)
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
									material_a: vector![(a_material as u8 & 0xC) >> 2, a_material as u8 & 0x3],
									material_b: vector![(b_material as u8 & 0xC) >> 2, b_material as u8 & 0x3],
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
					position: self.coordinates.coordinates.cast() * (16u64 << *self.coordinates.level) as f32,
					scale: (*self.coordinates.level + 1) as f32,
				}]),
				usage: BufferUsages::VERTEX,
			}),
		});
	}
}
