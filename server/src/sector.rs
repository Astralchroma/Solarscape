use crate::{config, generation::sphere_generator, generation::Generator, player::Connection, player::Player};
use dashmap::DashMap;
use log::{info, warn};
use nalgebra::{point, vector, Point3};
use rapier3d::dynamics::{
	CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet, RigidBodyBuilder,
	RigidBodyHandle, RigidBodySet,
};
use rapier3d::geometry::{ColliderBuilder, ColliderHandle, ColliderSet, DefaultBroadPhase, NarrowPhase};
use rapier3d::pipeline::PhysicsPipeline;
use solarscape_shared::messages::clientbound::{ClientboundMessage, SyncChunk};
use solarscape_shared::messages::serverbound::ServerboundMessage;
use solarscape_shared::triangulation_table::{EdgeData, CELL_EDGE_MAP, CORNERS, EDGE_CORNER_MAP};
use solarscape_shared::types::{ChunkCoordinates, Material, VoxjectId};
use std::sync::{atomic::AtomicUsize, atomic::Ordering::Relaxed, Arc, Weak};
use std::{array, collections::HashMap, mem, mem::MaybeUninit, ops::Deref, thread, time::Duration, time::Instant};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Represents an individual gameworld, generally it is only accessible on the sector thread, and is owned and managed
/// by the tick loop.
pub struct Sector {
	pub handle: Arc<SectorHandle>,

	events: Receiver<Event>,

	ticking_chunks: HashMap<ChunkCoordinates, TickingChunk>,
	players: HashMap<usize, Player>,

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

impl Sector {
	pub fn load(
		config::Sector { name, voxjects }: config::Sector,
		callback: impl Fn() + Send + 'static,
	) -> Arc<SectorHandle> {
		let start_time = Instant::now();

		let (sender, events) = channel();
		let handle = Arc::new(SectorHandle {
			name,
			sender,
			voxjects: voxjects.into_iter().map(Voxject::new).collect(),
			chunks: DashMap::new(),
		});
		let ret = handle.clone();

		thread::Builder::new()
			.name(handle.name.to_string())
			.spawn(move || {
				let sector = Self {
					handle,

					events,

					ticking_chunks: HashMap::new(),
					players: HashMap::new(),

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
				};

				let load_time = Instant::now() - start_time;
				info!("{:?} ready! {load_time:.0?}", sector.name);

				callback();

				sector.run();
			})
			.expect("failed to spawn thread");

		ret
	}

	fn run(mut self) {
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
				Event::PlayerConnected(connection) => {
					let player = Player::accept(self, connection);
					self.players.insert(player.id, player);
				}
				Event::PlayerDisconnected(id) => {
					self.players.remove(&id);
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
		for player in self.players.values_mut() {
			while let Ok(message) = player.try_recv() {
				match message {
					ServerboundMessage::PlayerLocation(location) => {
						// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
						player.location = location;

						let (new_client_locks, new_tick_locks) = player.compute_locks(&self.handle);

						let client_locks = new_client_locks
							.into_iter()
							.map(|coordinates| ClientLock::new(&self.handle, coordinates, player.connection.clone()))
							.collect();

						player.client_locks = client_locks;

						let tick_locks = new_tick_locks
							.into_iter()
							.map(|coordinates| TickLock::new(&self.handle, coordinates))
							.collect();

						player.tick_locks = tick_locks;
					}
				}
			}
		}
	}
}

impl Deref for Sector {
	type Target = Arc<SectorHandle>;

	fn deref(&self) -> &Self::Target {
		&self.handle
	}
}

/// A [`SectorHandle`] allows accessing shared information about a [`Sector`], as well as sending events to be
/// processed at the start of the next tick. It does not allow directly accessing the [`Sector`]'s internal state
/// however.
pub struct SectorHandle {
	/// The name of the [`Sector`] as specified by the server's configuration.
	pub name: Box<str>,

	sender: Sender<Event>,

	pub voxjects: HashMap<VoxjectId, Voxject>,
	chunks: DashMap<ChunkCoordinates, Weak<Chunk>>,
}

impl SectorHandle {
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

/// [`Event`]s are sent to [`Sector`]s and are processed at the start of the next tick.
pub enum Event {
	PlayerConnected(Arc<Connection>),
	PlayerDisconnected(usize),
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
	pub sector: Weak<SectorHandle>,
	pub coordinates: ChunkCoordinates,

	// This is deliberately a `Mutex<HashMap<K, V>>` instead of a `DashMap<K, V>` as when locking the chunk for a
	// connection, we need to prevent another thread from syncing at the same time, otherwise it could cause a desync.
	subscribed_clients: Mutex<HashMap<usize, Arc<Connection>>>,

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
	fn new(sector: &Arc<SectorHandle>, coordinates: ChunkCoordinates) -> Arc<Self> {
		let return_chunk = Arc::new(Self {
			sector: Arc::downgrade(sector),
			coordinates,

			subscribed_clients: Mutex::new(HashMap::new()),

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

		let message = ClientboundMessage::SyncChunk(SyncChunk {
			coordinates: self.coordinates,
			materials: data.as_ref().unwrap().materials.clone(),
			densities: data.as_ref().unwrap().densities.clone(),
		});

		self.subscribed_clients
			.blocking_lock()
			.values()
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
	collider: ColliderHandle,
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

			sector.colliders.insert_with_parent(
				// It hurts to have to call clone here.
				ColliderBuilder::trimesh(collision.vertices.clone(), collision.indices.clone()).build(),
				rigid_body,
				&mut sector.rigid_bodies,
			)
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
	chunks: [Arc<Chunk>; 27],
	id: usize,
}

impl ClientLock {
	pub fn new(sector: &Arc<SectorHandle>, coordinates: ChunkCoordinates, connection: Arc<Connection>) -> Self {
		// Build a list of chunks that we need to subscribe to
		let chunks: [Arc<Chunk>; 27] = {
			let mut chunks: [_; 27] = array::from_fn(|_| MaybeUninit::uninit());
			let mut index = 0;
			for x in -1..=1 {
				for y in -1..=1 {
					for z in -1..=1 {
						let coordinates = coordinates + vector![x, y, z];
						chunks[index] = MaybeUninit::new(sector.get_chunk(coordinates));
						index += 1;
					}
				}
			}
			unsafe { mem::transmute(chunks) }
		};

		for chunk in &chunks {
			let mut subscribed_clients = chunk.subscribed_clients.blocking_lock();

			// is_none check to avoid duplicate chunk syncs
			if subscribed_clients.insert(connection.id, connection.clone()).is_none() {
				if let Some(ref data) = *chunk.try_read_data() {
					connection.send(SyncChunk {
						coordinates: chunk.coordinates,
						materials: data.materials.clone(),
						densities: data.densities.clone(),
					});
				}
			}
		}

		Self {
			chunks,
			id: connection.id,
		}
	}
}

impl Drop for ClientLock {
	fn drop(&mut self) {
		for chunk in &self.chunks {
			chunk.subscribed_clients.blocking_lock().remove(&self.id);
		}
	}
}

pub struct TickLock([Arc<Chunk>; 27]);

impl TickLock {
	pub fn new(sector: &Arc<SectorHandle>, coordinates: ChunkCoordinates) -> Self {
		// Build a list of chunks that we need to lock
		let chunks: [Arc<Chunk>; 27] = {
			let mut chunks: [_; 27] = array::from_fn(|_| MaybeUninit::uninit());
			let mut index = 0;
			for x in -1..=1 {
				for y in -1..=1 {
					for z in -1..=1 {
						let coordinates = coordinates + vector![x, y, z];
						chunks[index] = MaybeUninit::new(sector.get_chunk(coordinates));
						index += 1;
					}
				}
			}
			unsafe { mem::transmute(chunks) }
		};

		for chunk in &chunks {
			if chunk.tick_lock_count.fetch_add(1, Relaxed) == 0 {
				let _ = sector.send(Event::TickLockChunk(chunk.coordinates));
				chunk.clone().trigger_collision_mesh_rebuild();
			}
		}

		Self(chunks)
	}
}

impl Drop for TickLock {
	fn drop(&mut self) {
		for chunk in &self.0 {
			if chunk.tick_lock_count.fetch_sub(1, Relaxed) == 1 {
				if let Some(sector) = Weak::upgrade(&chunk.sector) {
					let _ = sector.send(Event::TickReleaseChunk(chunk.coordinates));
				}
			}
		}
	}
}
