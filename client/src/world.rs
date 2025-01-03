use crate::{
	client::{AnyState, State},
	player::{Local, Player},
};
use bytemuck::{cast_slice, Pod, Zeroable};
use dashmap::DashMap;
use egui::{Align::Min, Align2, Layout, Window};
use log::debug;
use nalgebra::{point, vector, Isometry3, Vector2, Vector3};
use rapier3d::{
	dynamics::{RigidBodyBuilder, RigidBodyHandle},
	geometry::{ColliderBuilder, ColliderHandle},
};
use rustc_hash::FxBuildHasher;
use solarscape_shared::{
	connection::{ClientEnd, Connection},
	data::{
		world::{ChunkCoordinates, Material, LEVELS},
		Id,
	},
	message::{
		clientbound::{Clientbound, InventorySlot, RemoveChunk, Sync, SyncChunk, SyncInventory},
		serverbound::Serverbound,
	},
	physics::{AutoCleanup, Physics},
	structure::Structure,
	triangulation_table::{EdgeData, CELL_EDGE_MAP, CORNERS, EDGE_CORNER_MAP},
};
use std::{
	collections::{HashMap, HashSet},
	fmt::Write,
	mem::drop as nom,
	ops::Deref,
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::sync::mpsc::error::TryRecvError;
use wgpu::{
	util::{BufferInitDescriptor, DeviceExt},
	Buffer, BufferUsages, Device,
};
use winit::{
	event::{DeviceEvent, ElementState, KeyEvent, WindowEvent},
	keyboard::{KeyCode, PhysicalKey},
};

pub struct Sector {
	shared: Arc<SharedSector>,

	pub player: Player<Local>,

	inventory: Vec<InventorySlot>,
	pub inventory_gui_open: bool,

	pub structures: Vec<Structure>,
	pub voxjects: HashMap<Id, Voxject>,

	last_tick_start: Instant,

	pub physics: Physics,
}

pub struct SharedSector {
	pub chunks: DashMap<ChunkCoordinates, Chunk, FxBuildHasher>,
	pub dependent_chunks: DashMap<ChunkCoordinates, HashSet<ChunkCoordinates>, FxBuildHasher>,
}

impl Sector {
	pub async fn new(mut connection: Connection<ClientEnd>) -> Self {
		let Sync {
			voxjects,
			structures,
			inventory,
			..
		} = loop {
			let message = connection.recv().await.expect("server should respond");

			match message {
				Clientbound::Sync(sync_sector) => break sync_sector,
				_ => continue,
			};
		};

		let player = Player::new(connection);
		let mut physics = Physics::new();

		Self {
			shared: Arc::new(SharedSector {
				chunks: DashMap::with_hasher(FxBuildHasher),
				dependent_chunks: DashMap::with_hasher(FxBuildHasher),
			}),

			player,

			inventory,
			inventory_gui_open: false,

			voxjects: voxjects
				.into_iter()
				.map(|voxject| {
					(
						voxject.id,
						Voxject {
							id: voxject.id,
							name: voxject.name,
							location: Isometry3::default(),
						},
					)
				})
				.collect(),
			structures: structures
				.into_iter()
				.map(|sync_structure| Structure::new_from_sync(&mut physics, sync_structure))
				.collect(),

			last_tick_start: Instant::now(),

			physics,
		}
	}

	pub fn process_messages(&mut self, device: &Device) {
		let start_time = Instant::now();

		loop {
			if Instant::now() - start_time >= Duration::from_secs(1) {
				break;
			}

			let message = match self.player.connection.try_recv() {
				Ok(message) => message,
				Err(TryRecvError::Disconnected) => panic!("disconnected"),
				Err(TryRecvError::Empty) => return,
			};

			match message {
				Clientbound::Sync(_) => continue, // what...?
				Clientbound::SyncInventory(SyncInventory(inventory)) => self.inventory = inventory,
				Clientbound::SyncChunk(SyncChunk {
					coordinates,
					materials,
					densities,
				}) => self.add_chunk(
					device,
					Chunk {
						coordinates,
						materials,
						densities,
						mesh: None,
					},
				),
				Clientbound::RemoveChunk(RemoveChunk(coordinates)) => {
					self.remove_chunk(device, coordinates)
				}
				Clientbound::SyncStructure(sync_structure) => {
					debug!("Synced structure {}", sync_structure.id);
					self.structures
						.push(Structure::new_from_sync(&mut self.physics, sync_structure));
				}
			}
		}
	}

	pub fn add_chunk(&mut self, device: &Device, chunk: Chunk) {
		let coordinates = chunk.coordinates;
		self.chunks.insert(coordinates, chunk);

		// Rebuild any chunks that need this chunk
		'rebuild_dependents: {
			let dependent_chunks = match self.dependent_chunks.get(&coordinates) {
				Some(dependent_chunks) => dependent_chunks.clone(),
				None => break 'rebuild_dependents,
			};

			for dependent_chunk in dependent_chunks {
				self.try_build_chunk(device, dependent_chunk);
			}
		}

		self.try_build_chunk(device, coordinates);
	}

	pub fn remove_chunk(&mut self, device: &Device, coordinates: ChunkCoordinates) {
		self.chunks.remove(&coordinates);

		let dependent_chunks = match self.dependent_chunks.get(&coordinates) {
			Some(dependent_chunks) => dependent_chunks.clone(),
			None => return,
		};

		for dependent_chunk in dependent_chunks {
			self.try_build_chunk(device, dependent_chunk);
		}
	}

	// This code is admittedly absolutely fucking terrible, for the time being I don't care, it just needs to work
	pub fn try_build_chunk(&mut self, device: &Device, grid_coordinates: ChunkCoordinates) {
		let dependency_grid_coordinates = [
			grid_coordinates + Vector3::new(0, 0, 0),
			grid_coordinates + Vector3::new(0, 0, 1),
			grid_coordinates + Vector3::new(0, 1, 0),
			grid_coordinates + Vector3::new(0, 1, 1),
			grid_coordinates + Vector3::new(1, 0, 0),
			grid_coordinates + Vector3::new(1, 0, 1),
			grid_coordinates + Vector3::new(1, 1, 0),
			grid_coordinates + Vector3::new(1, 1, 1),
		];

		let dependency_chunks =
			dependency_grid_coordinates.map(|coordinates| self.chunks.get(&coordinates));

		let mut upleveled_dependency_grid_coordinates = None;
		let mut upleveled_dependency_chunks = Default::default();

		let should_uplevel = *grid_coordinates.level != LEVELS - 2;
		if should_uplevel {
			upleveled_dependency_grid_coordinates =
				Some(dependency_grid_coordinates.map(|coordinates| coordinates.upleveled()));
			upleveled_dependency_chunks = upleveled_dependency_grid_coordinates
				.unwrap()
				.map(|coordinates| self.chunks.get(&coordinates));
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
					if let Some(chunk) = &dependency_chunks[chunk_index] {
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
						let upleveled_chunk_index =
							((u_x & 0x10) >> 2) | ((u_y & 0x10) >> 3) | ((u_z & 0x10) >> 4);

						if let Some(chunk) = &upleveled_dependency_chunks[upleveled_chunk_index] {
							let u_chunk_cell_index =
								(u_x & 0x0F) << 8 | (u_y & 0x0F) << 4 | u_z & 0x0F;
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
			match self.dependent_chunks.get_mut(&level_coordinates) {
				None => {
					self.dependent_chunks
						.insert(level_coordinates, HashSet::from([grid_coordinates]));
				}
				Some(mut dependent_chunks) => {
					dependent_chunks.value_mut().insert(grid_coordinates);
				}
			}
		}

		if should_uplevel {
			// Now either add or remove our dependency on upleveled chunks
			for level_coordinates in upleveled_dependency_grid_coordinates.unwrap() {
				let should_remove = match self.dependent_chunks.get_mut(&level_coordinates) {
					None if need_upleveled_chunks => {
						self.dependent_chunks.insert(
							level_coordinates,
							HashSet::from([upleveled_grid_coordinates]),
						);
						false
					}
					Some(mut dependent_chunks) => {
						match need_upleveled_chunks {
							true => dependent_chunks.insert(upleveled_grid_coordinates),
							false => dependent_chunks.remove(&upleveled_grid_coordinates),
						};

						dependent_chunks.is_empty()
					}
					_ => false,
				};

				if should_remove {
					self.dependent_chunks.remove(&level_coordinates);
				}
			}
		}

		nom(dependency_chunks);
		nom(upleveled_dependency_chunks);

		let shared_clone = self.shared.clone();
		if let Some(mut chunk) = shared_clone.chunks.get_mut(&grid_coordinates) {
			// Not enough data to build chunk
			if need_upleveled_chunks {
				chunk.value_mut().mesh = None;
				return;
			}

			// Now we can build the chunk mesh
			chunk.rebuild_mesh(self, device, densities, materials);
		};
	}
}

impl State for Sector {
	fn tick(&mut self) -> Option<AnyState> {
		let tick_start = Instant::now();
		let delta = (tick_start - self.last_tick_start).as_secs_f32();
		self.last_tick_start = tick_start;

		self.player.tick(delta);

		self.physics.tick(delta);

		None
	}

	fn build_debug_text(&mut self, debug_text: &mut String) {
		let (x, y, z) = self.player.location.rotation.euler_angles();

		writeln!(
			debug_text,
			"{:.2?} [{:.2?}, {:.2?}, {:.2?}]",
			self.player.location.position,
			f32::to_degrees(x),
			f32::to_degrees(y),
			f32::to_degrees(z),
		)
		.expect("should be able to write to string");

		writeln!(debug_text, "Structures: {}", self.structures.len())
			.expect("should be able to write to string");
		writeln!(
			debug_text,
			"Blocks: {}",
			self.structures
				.iter()
				.map(|structure| structure.num_blocks())
				.count()
		)
		.expect("should be able to write to string");
	}

	fn draw_ui(&mut self, _: &crate::ClArgs, context: &egui::Context) {
		Window::new("Inventory")
			.anchor(Align2::CENTER_CENTER, [0.0, 0.0])
			.auto_sized()
			.collapsible(false)
			.hscroll(false)
			.max_width(512.0)
			.open(&mut self.inventory_gui_open)
			.resizable(false)
			.show(context, |window| {
				if window
					.button(r#"Temporary magic "give me an item" button"#)
					.clicked()
				{
					self.player.connection.send(Serverbound::GiveTestItem);
				}

				window.columns(4, |columns| {
					let mut column = 0;

					for InventorySlot { item, quantity } in &self.inventory {
						let next_column = {
							let result = column;
							column += 1;
							if column == columns.len() {
								column = 0;
							}
							result
						};

						columns[next_column].group(|group| {
							group.with_layout(Layout::top_down(Min), |group| {
								group.label(format!("{} ({})", item.display_name(), quantity));
								group.label(item.description());
							});
						});
					}
				});
			});
	}

	fn window_event(&mut self, event: &WindowEvent) {
		match self.inventory_gui_open {
			true => {
				if let WindowEvent::KeyboardInput {
					event:
						KeyEvent {
							physical_key: PhysicalKey::Code(KeyCode::Escape),
							state: ElementState::Released,
							repeat: false,
							..
						},
					..
				} = event
				{
					self.inventory_gui_open = false;
				}
			}
			false => {
				if let WindowEvent::KeyboardInput {
					event:
						KeyEvent {
							physical_key: PhysicalKey::Code(KeyCode::Tab),
							state: ElementState::Released,
							repeat: false,
							..
						},
					..
				} = event
				{
					self.inventory_gui_open = true;
				} else {
					self.player.handle_window_event(event);
				}
			}
		}
	}

	fn device_event(&mut self, event: &DeviceEvent) {
		if !self.inventory_gui_open {
			self.player.handle_device_event(event);
		}
	}
}

impl Deref for Sector {
	type Target = SharedSector;

	fn deref(&self) -> &Self::Target {
		&self.shared
	}
}

pub struct Voxject {
	pub id: Id,
	pub name: Box<str>,
	pub location: Isometry3<f32>,
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

	pub vertex_position_buffer: Buffer,
	pub vertex_data_buffer: Buffer,
	pub instance_buffer: Buffer,

	collider: AutoCleanup<ColliderHandle>,
	rigid_body: AutoCleanup<RigidBodyHandle>,
}

#[allow(unused)]
#[derive(Clone, Copy)]
#[repr(packed)]
struct VertexData {
	normal: Vector3<f32>,
	material_a: Vector2<u8>,
	material_b: Vector2<u8>,
	weight: f32,
}

impl Chunk {
	pub fn rebuild_mesh(
		&mut self,
		sector: &mut Sector,
		device: &Device,
		densities: [f32; 17 * 17 * 17],
		materials: [Material; 17 * 17 * 17],
	) {
		let mut vertex_positions = vec![];
		let mut vertex_data = vec![];

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

					let EdgeData {
						count,
						edge_indices,
					} = CELL_EDGE_MAP[case_index];

					for edge_indices in edge_indices.chunks(3).take(count as usize) {
						let mut cell_vertex_positions = vec![];
						let mut cell_vertex_data = vec![];

						for edge_index in edge_indices.iter() {
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

							cell_vertex_positions
								.push(point![x as f32, y as f32, z as f32] + vertex);

							cell_vertex_data.push(VertexData {
								normal: Vector3::default(),
								material_a: vector![
									(a_material as u8 & 0xC) >> 2,
									a_material as u8 & 0x3
								],
								material_b: vector![
									(b_material as u8 & 0xC) >> 2,
									b_material as u8 & 0x3
								],
								weight,
							});
						}

						let normal = (cell_vertex_positions[1] - cell_vertex_positions[0])
							.cross(&(cell_vertex_positions[2] - cell_vertex_positions[0]))
							.normalize();

						cell_vertex_data[0].normal = normal;
						cell_vertex_data[1].normal = normal;
						cell_vertex_data[2].normal = normal;

						vertex_positions.extend_from_slice(&cell_vertex_positions);
						vertex_data.extend_from_slice(&cell_vertex_data);
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

		let rigid_body = sector.physics.insert_rigid_body(
			RigidBodyBuilder::fixed().translation(self.coordinates.voxject_relative_translation()),
		);

		let vertex_indices = (0..vertex_positions.len() as u32)
			.collect::<Vec<_>>()
			.chunks_exact(3)
			.map(|chunk| [chunk[0], chunk[1], chunk[2]])
			.collect();

		self.mesh = Some(ChunkMesh {
			vertex_count: vertex_data.len() as u32,

			vertex_position_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh#vertex_position_buffer"),
				contents: cast_slice(&vertex_positions),
				usage: BufferUsages::VERTEX,
			}),
			vertex_data_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh#vertex_data_buffer"),
				contents: cast_slice(&vertex_data),
				usage: BufferUsages::VERTEX,
			}),
			instance_buffer: device.create_buffer_init(&BufferInitDescriptor {
				label: Some("chunk.mesh.instance_buffer"),
				contents: cast_slice(&[InstanceData {
					position: self.coordinates.coordinates.cast()
						* (16u64 << *self.coordinates.level) as f32,
					scale: (*self.coordinates.level + 1) as f32,
				}]),
				usage: BufferUsages::VERTEX,
			}),

			collider: sector.physics.insert_rigid_body_collider(
				*rigid_body,
				ColliderBuilder::trimesh(vertex_positions, vertex_indices),
			),
			rigid_body,
		});
	}
}
