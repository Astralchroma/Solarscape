use crate::{generation::sphere_generator, generation::Generator, player::Player};
use dashmap::DashMap;
use log::warn;
use nalgebra::{coordinates, point, vector, Point3};
use rapier3d::dynamics::{
	CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet, RigidBodyBuilder,
	RigidBodyHandle, RigidBodySet,
};
use rapier3d::geometry::{ColliderBuilder, ColliderHandle, ColliderSet, DefaultBroadPhase, NarrowPhase};
use rapier3d::pipeline::PhysicsPipeline;
use solarscape_backend_types::types::Id;
use solarscape_shared::connection::{Connection, ConnectionSend, ServerEnd};
use solarscape_shared::message::{Clientbound, Serverbound, SyncChunk};
use solarscape_shared::triangulation_table::{EdgeData, CELL_EDGE_MAP, CORNERS, EDGE_CORNER_MAP};
use solarscape_shared::types::{ChunkCoordinates, Material, VoxjectId};
use std::sync::{atomic::AtomicUsize, atomic::Ordering::Relaxed, Arc, Weak};
use std::{collections::HashMap, mem::drop as nom, ops::Deref, thread, time::Duration, time::Instant};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub mod config {
	use serde::Deserialize;

	#[derive(Deserialize)]
	pub struct Sector {
		pub name: Box<str>,
		pub voxjects: Vec<Voxject>,
	}

	#[derive(Deserialize)]
	pub struct Voxject {
		pub name: Box<str>,
	}
}

pub struct Sector {
	pub shared: Arc<SharedSector>,

	events: Receiver<Event>,

	ticking_chunks: HashMap<ChunkCoordinates, TickingChunk>,
	players: Vec<Player>,

	// A bunch of different data used by Rapier, most of it isn't important
	physics_pipeline: PhysicsPipeline,
	rigid_bodies: RigidBodySet,
	integration_parameters: IntegrationParameters,
	islands: IslandManager,
	broad_phase: DefaultBroadPhase,
	narrow_phase: NarrowPhase,
	colliders: ColliderSet,
	impulse_joints: ImpulseJointSet,
	multibody_joints: MultibodyJointSet,
	ccd_solver: CCDSolver,
}

/// A [`SharedSector`] allows accessing shared information about a [`Sector`], as well as sending events to be
/// processed at the start of the next tick. It does not allow directly accessing the [`Sector`]'s internal state
/// however.
pub struct SharedSector {
	pub name: Box<str>,

	sender: Sender<Event>,

	pub voxjects: HashMap<VoxjectId, Voxject>,
	chunks: DashMap<ChunkCoordinates, Weak<Chunk>>,
}

impl From<config::Sector> for Sector {
	fn from(config::Sector { name, voxjects }: config::Sector) -> Self {
		let (sender, events) = channel();

		Self {
			shared: Arc::new(SharedSector {
				name,
				sender,
				voxjects: voxjects.into_iter().map(Voxject::new).collect(),
				chunks: DashMap::new(),
			}),

			events,

			ticking_chunks: HashMap::new(),
			players: vec![],

			physics_pipeline: PhysicsPipeline::new(),
			integration_parameters: IntegrationParameters::default(),
			islands: IslandManager::new(),
			broad_phase: DefaultBroadPhase::new(),
			narrow_phase: NarrowPhase::new(),
			rigid_bodies: RigidBodySet::new(),
			colliders: ColliderSet::new(),
			impulse_joints: ImpulseJointSet::new(),
			multibody_joints: MultibodyJointSet::new(),
			ccd_solver: CCDSolver::new(),
		}
	}
}

impl Sector {
	pub fn run(mut self) {
		let target_tick_time = Duration::from_secs(1) / 30;

		loop {
			let tick_start = Instant::now();

			self.tick();

			let tick_duration = Instant::now() - tick_start;

			match target_tick_time.checked_sub(tick_duration) {
				Some(time_until_next_tick) => thread::sleep(time_until_next_tick),
				None => warn!("Tick took {tick_duration:.0?}, exceeding {target_tick_time:.0?} target"),
			}
		}
	}

	fn tick(&mut self) {
		self.handle_events();
		self.process_players();

		self.physics_pipeline.step(
			&vector![0.0, 0.0, 0.0],
			&self.integration_parameters,
			&mut self.islands,
			&mut self.broad_phase,
			&mut self.narrow_phase,
			&mut self.rigid_bodies,
			&mut self.colliders,
			&mut self.impulse_joints,
			&mut self.multibody_joints,
			&mut self.ccd_solver,
			None,
			&(),
			&(),
		);
	}

	fn handle_events(&mut self) {
		while let Ok(event) = self.events.try_recv() {
			match event {
				Event::PlayerConnected(_, connection) => {
					let player = Player::accept(self, connection);
					self.players.push(player);
				}
				Event::TickLockChunk(coordinates) => {
					let chunk = self.get_chunk(coordinates);
					TickingChunk::register(self, chunk);
				}
				Event::TickReleaseChunk(coordinates) => {
					self.ticking_chunks.remove(&coordinates);
				}
			}
		}
	}

	pub fn process_players(&mut self) {
		self.players.retain(|player| player.connection.is_connected());

		for player in self.players.iter_mut() {
			while let Ok(message) = player.try_recv() {
				match message {
					Serverbound::PlayerLocation(location) => {
						// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
						player.location = location;

						let (mut new_client_locks, mut new_tick_locks) = player.compute_locks(&self.shared);

						player
							.client_locks
							// Retain will remove any chunks that arent in the new list, remove will
							// remove any chunks from the new list that were in the old list
							.retain(|lock| new_client_locks.remove(&lock.chunk.coordinates));

						for coordinates in new_client_locks {
							player.client_locks.push(ClientLock::new(
								&self.shared,
								coordinates,
								player.connection.sender(),
							));
						}

						// Same as before, though there probably isn't a performence gain to doing it here
						player
							.tick_locks
							.retain(|lock| new_tick_locks.remove(&lock.0.coordinates));

						for coordinates in new_tick_locks {
							player.tick_locks.push(TickLock::new(&self.shared, coordinates));
						}
					}
				}
			}
		}
	}
}

impl SharedSector {
	/// Sends an event to the [`Sector`] to be processed at the start of the next tick. The event is returned if the
	/// event could not be sent.
	pub fn send(&self, event: Event) -> Result<(), Event> {
		self.sender.send(event).map_err(|error| error.0)
	}

	pub fn get_chunk(self: &Arc<Self>, coordinates: ChunkCoordinates) -> Arc<Chunk> {
		self.chunks
			.get(&coordinates)
			.as_deref()
			.and_then(Weak::upgrade)
			.unwrap_or_else(|| {
				let chunk = Chunk::new(self, coordinates);
				self.chunks.insert(coordinates, Arc::downgrade(&chunk));
				chunk
			})
	}
}

impl Deref for Sector {
	type Target = Arc<SharedSector>;

	fn deref(&self) -> &Self::Target {
		&self.shared
	}
}

/// [`Event`]s are sent to [`Sector`]s and are processed at the start of the next tick.
pub enum Event {
	PlayerConnected(Id, Connection<ServerEnd>),
	TickLockChunk(ChunkCoordinates),
	TickReleaseChunk(ChunkCoordinates),
}

pub struct Voxject {
	pub id: VoxjectId,
	pub name: Box<str>,
	pub generator: Generator,
}

impl Voxject {
	pub fn new(config::Voxject { name }: config::Voxject) -> (VoxjectId, Self) {
		let id = VoxjectId::new();
		let voxject = Self {
			id,
			name,
			generator: sphere_generator,
		};
		(id, voxject)
	}
}

#[non_exhaustive]
pub struct Chunk {
	pub sector: Weak<SharedSector>,
	pub coordinates: ChunkCoordinates,

	subscribed_clients: Mutex<Vec<Arc<ConnectionSend<ServerEnd>>>>,

	// Multiple tick locks may exist, we need to avoid removing a chunk from the ticking list if its tick locked
	// elsewhere.
	tick_lock_count: AtomicUsize,

	data: RwLock<Option<Data>>,
	collision: RwLock<Option<Collision>>,
}

pub type DataTryReadGuard<'a> = RwLockReadGuard<'a, Option<Data>>;
pub type DataReadGuard<'a> = RwLockReadGuard<'a, Data>;

pub type CollisionReadGuard<'a> = RwLockReadGuard<'a, Collision>;

impl Chunk {
	fn new(sector: &Arc<SharedSector>, coordinates: ChunkCoordinates) -> Arc<Self> {
		let return_chunk = Arc::new(Self {
			sector: Arc::downgrade(sector),
			coordinates,

			subscribed_clients: Mutex::new(vec![]),

			tick_lock_count: AtomicUsize::new(0),

			data: RwLock::default(),
			collision: RwLock::default(),
		});

		let chunk = return_chunk.clone();
		rayon::spawn(move || {
			// If try_unwrap returns Ok then nothing else wanted the chunk, so to avoid doing work that will be
			// immidately discarded, we only generate the chunk if we cannot take exclusive ownership of it.
			if let Err(chunk) = Arc::try_unwrap(chunk) {
				let data = chunk.data.blocking_write();
				let _ = chunk.generate_data(data);
			}
		});

		return_chunk
	}

	fn generate_data<'a>(&'a self, mut data: RwLockWriteGuard<'a, Option<Data>>) -> RwLockReadGuard<'a, Option<Data>> {
		// Another thread may synchronously generate chunks instead of waiting if the chunk is needed
		// immediately. So if that has happened, don't re-generate the chunk.
		if data.is_some() {
			return data.downgrade();
		}

		let generator = self
			.sector
			.upgrade()
			.expect("Chunk should not be used after Sector has been dropped")
			.voxjects[&self.coordinates.voxject]
			.generator;

		*data = Some(generator(&self.coordinates));

		let data = data.downgrade();

		let message = Clientbound::SyncChunk(SyncChunk {
			coordinates: self.coordinates,
			materials: data.as_ref().unwrap().materials.clone(),
			densities: data.as_ref().unwrap().densities.clone(),
		});

		self.subscribed_clients
			.blocking_lock()
			.iter()
			.for_each(|connection| connection.send(message.clone()));

		data
	}

	fn generate_collision<'a>(
		self: &'a Arc<Self>,
		mut collision: RwLockWriteGuard<'a, Option<Collision>>,
	) -> RwLockReadGuard<'a, Option<Collision>> {
		if collision.is_some() {
			return collision.downgrade();
		}

		let sector = self
			.sector
			.upgrade()
			.expect("Chunk should not be used after Sector has been dropped");

		let chunks = [
			self.clone(),
			sector.get_chunk(self.coordinates + vector![0, 0, 1]),
			sector.get_chunk(self.coordinates + vector![0, 1, 0]),
			sector.get_chunk(self.coordinates + vector![0, 1, 1]),
			sector.get_chunk(self.coordinates + vector![1, 0, 0]),
			sector.get_chunk(self.coordinates + vector![1, 0, 1]),
			sector.get_chunk(self.coordinates + vector![1, 1, 0]),
			sector.get_chunk(self.coordinates + vector![1, 1, 1]),
		];

		let chunk_data_guards = chunks.each_ref().map(|chunk| chunk.read_data_immediately());

		let mut densities = [0f32; usize::pow(17, 3)];
		let mut materials = [Material::Nothing; usize::pow(17, 3)];

		for x in 0..17 {
			for y in 0..17 {
				for z in 0..17 {
					let chunk_index = ((x & 0x10) >> 2) | ((y & 0x10) >> 3) | ((z & 0x10) >> 4);
					let cell_index = (x * 17 * 17) + (y * 17) + z;
					let chunk_cell_index = (x & 0x0F) << 8 | (y & 0x0F) << 4 | z & 0x0F;

					densities[cell_index] = chunk_data_guards[chunk_index].densities[chunk_cell_index];
					materials[cell_index] = chunk_data_guards[chunk_index].materials[chunk_cell_index];
				}
			}
		}

		let mut new_collision = Collision::default();

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
						let vertices = edge_indices
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

								point![x as f32, y as f32, z as f32] + vertex
							})
							.collect::<Vec<_>>();

						new_collision.vertices.extend_from_slice(&vertices);
					}
				}
			}
		}

		new_collision.indices = (0..new_collision.vertices.len() as u32)
			.collect::<Vec<_>>()
			.chunks_exact(3)
			.map(|chunk| [chunk[0], chunk[1], chunk[2]])
			.collect();

		*collision = Some(new_collision);
		return collision.downgrade();
	}

	pub fn read_data_immediately(&self) -> DataReadGuard {
		{
			let data = self.data.blocking_read();

			if data.is_some() {
				return RwLockReadGuard::map(data, |v| v.as_ref().unwrap());
			}
		}

		let data = self.generate_data(self.data.blocking_write());
		RwLockReadGuard::map(data, |v| v.as_ref().unwrap())
	}

	pub fn read_collision_immediately<'a>(self: &'a Arc<Chunk>) -> CollisionReadGuard<'a> {
		{
			let collision = self.collision.blocking_read();

			if collision.is_some() {
				return RwLockReadGuard::map(collision, |v| v.as_ref().unwrap());
			}
		}

		let collision = self.generate_collision(self.collision.blocking_write());
		RwLockReadGuard::map(collision, |v| v.as_ref().unwrap())
	}

	pub fn try_read_data(&self) -> DataTryReadGuard {
		self.data.blocking_read()
	}

	pub fn trigger_collision_mesh_rebuild(self: Arc<Self>) {
		rayon::spawn(move || {
			// If try_unwrap returns Ok then nothing else wanted the chunk, so to avoid doing work that will be
			// immidately discarded, we only generate the chunk's collision mesh if we cannot take exclusive ownership of it.
			if let Err(chunk) = Arc::try_unwrap(self) {
				let collision = chunk.collision.blocking_write();
				let _ = chunk.generate_collision(collision);
			}
		});
	}
}

impl Drop for Chunk {
	fn drop(&mut self) {
		if let Some(sector) = Weak::upgrade(&self.sector) {
			sector.chunks.remove(&self.coordinates);
		}
	}
}

/// A wrapper around [`Chunk`] that stores extra information needed to allow the chunk to tick, and should not be
/// accessible outside of the sector thread.
struct TickingChunk {
	inner: Arc<Chunk>,
	rigid_body: RigidBodyHandle,
	collider: Option<ColliderHandle>,
}

impl TickingChunk {
	fn register(sector: &mut Sector, chunk: Arc<Chunk>) {
		let rigid_body = sector.rigid_bodies.insert(
			RigidBodyBuilder::fixed()
				.translation(chunk.coordinates.voxject_relative_translation())
				.build(),
		);

		let collider = {
			let collision = chunk.read_collision_immediately();

			match collision.vertices.is_empty() {
				true => None,
				false => Some(sector.colliders.insert_with_parent(
					// It hurts to have to call clone here.
					ColliderBuilder::trimesh(collision.vertices.clone(), collision.indices.clone()).build(),
					rigid_body,
					&mut sector.rigid_bodies,
				)),
			}
		};

		let ticking_chunk = Self {
			inner: chunk,
			rigid_body,
			collider,
		};

		sector.ticking_chunks.insert(ticking_chunk.coordinates, ticking_chunk);
	}
}

impl Deref for TickingChunk {
	type Target = Arc<Chunk>;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

#[non_exhaustive]
pub struct Data {
	pub materials: Box<[Material; 4096]>,
	pub densities: Box<[f32; 4096]>,
}

impl Default for Data {
	fn default() -> Self {
		Self {
			materials: Box::new([Material::Nothing; 4096]),
			densities: Box::new([0.0; 4096]),
		}
	}
}

#[derive(Default)]
#[non_exhaustive]
pub struct Collision {
	pub vertices: Vec<Point3<f32>>,
	pub indices: Vec<[u32; 3]>,
}

pub struct ClientLock {
	chunk: Arc<Chunk>,
	connection: Arc<ConnectionSend<ServerEnd>>,
}

impl ClientLock {
	pub fn new(
		sector: &Arc<SharedSector>,
		coordinates: ChunkCoordinates,
		connection: Arc<ConnectionSend<ServerEnd>>,
	) -> Self {
		let chunk = sector.get_chunk(coordinates);

		let mut subscribed_clients = chunk.subscribed_clients.blocking_lock();

		// is_none check to avoid duplicate chunk syncs
		if !subscribed_clients.contains(&connection) {
			subscribed_clients.push(connection.clone());
			if let Some(ref data) = *chunk.try_read_data() {
				connection.send(SyncChunk {
					coordinates: chunk.coordinates,
					materials: data.materials.clone(),
					densities: data.densities.clone(),
				});
			}
		}

		nom(subscribed_clients);

		Self { chunk, connection }
	}
}

impl Drop for ClientLock {
	fn drop(&mut self) {
		self.chunk
			.subscribed_clients
			.blocking_lock()
			.retain(|other| self.connection != *other);
	}
}

pub struct TickLock(Arc<Chunk>);

impl TickLock {
	pub fn new(sector: &Arc<SharedSector>, coordinates: ChunkCoordinates) -> Self {
		let chunk = sector.get_chunk(coordinates);

		if chunk.tick_lock_count.fetch_add(1, Relaxed) == 0 {
			let _ = sector.send(Event::TickLockChunk(chunk.coordinates));
			chunk.clone().trigger_collision_mesh_rebuild();
		}

		Self(chunk)
	}
}

impl Drop for TickLock {
	fn drop(&mut self) {
		if self.0.tick_lock_count.fetch_sub(1, Relaxed) == 1 {
			if let Some(sector) = Weak::upgrade(&self.0.sector) {
				let _ = sector.send(Event::TickReleaseChunk(self.0.coordinates));
			}
		}
	}
}
