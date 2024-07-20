use crate::{config, generation::sphere_generator, generation::Generator, player::Connection, player::Player};
use dashmap::DashMap;
use log::{error, warn};
use nalgebra::{point, vector, Point3};
use solarscape_shared::messages::clientbound::{ClientboundMessage, SyncChunk};
use solarscape_shared::triangulation_table::{EdgeData, CELL_EDGE_MAP, CORNERS, EDGE_CORNER_MAP};
use solarscape_shared::types::{ChunkCoordinates, Material, VoxjectId};
use std::sync::{atomic::AtomicUsize, atomic::Ordering::Relaxed, Arc, Weak};
use std::{array, collections::HashMap, mem, mem::MaybeUninit, thread, time::Duration, time::Instant};
use thiserror::Error;
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver as Receiver};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct Sector {
	pub name: Box<str>,

	voxjects: HashMap<VoxjectId, Voxject>,
	chunks: DashMap<ChunkCoordinates, Weak<Chunk>>,
	ticking_chunks: DashMap<usize, Arc<Chunk>>,

	players: Mutex<Vec<Player>>,

	connecting_players: Mutex<Receiver<Arc<Connection>>>,
}

impl Sector {
	#[must_use]
	pub fn new(
		config::Sector { name, voxjects }: config::Sector,
		connecting_players: Receiver<Arc<Connection>>,
	) -> Arc<Self> {
		let voxjects = voxjects.into_iter().map(Voxject::new).collect();

		Arc::new(Self {
			name,

			voxjects,
			chunks: DashMap::new(),
			ticking_chunks: DashMap::new(),

			players: Mutex::new(Vec::new()),

			connecting_players: Mutex::new(connecting_players),
		})
	}

	pub fn voxjects(&self) -> impl Iterator<Item = &Voxject> {
		self.voxjects.values()
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

	pub fn run(self: Arc<Self>) {
		let target_tick_time = Duration::from_secs(1) / 30;

		loop {
			let tick_start = Instant::now();

			if let Err(error) = self.tick() {
				error!("Fatal error occurred while ticking sector, stopping.\n{error}");
				break;
			}

			let tick_end = Instant::now();
			let tick_duration = tick_end - tick_start;

			match target_tick_time.checked_sub(tick_duration) {
				Some(time_until_next_tick) => thread::sleep(time_until_next_tick),
				None => warn!("Tick took {tick_duration:.0?}, exceeding {target_tick_time:.0?} target"),
			}
		}
	}

	fn tick(self: &Arc<Self>) -> Result<(), SectorTickError> {
		{
			let mut players = self.players.blocking_lock();

			// Remove disconnected players
			players.retain(|player| !player.is_disconnected());
		}

		// Handle connecting players
		loop {
			match self.connecting_players.blocking_lock().try_recv() {
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => return Err(SectorTickError::Dropped),
				Ok(connecting_player) => {
					let player = Player::accept(self, connecting_player);
					self.players.blocking_lock().push(player);
				}
			}
		}

		// Process all players
		for player in self.players.blocking_lock().iter() {
			player.process_player(self)
		}

		Ok(())
	}
}

#[derive(Debug, Error)]
enum SectorTickError {
	#[error("sector handle was dropped")]
	Dropped,
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
	pub id: usize,
	pub sector: Weak<Sector>,
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

impl Chunk {
	fn new(sector: &Arc<Sector>, coordinates: ChunkCoordinates) -> Arc<Self> {
		static COUNTER: AtomicUsize = AtomicUsize::new(0);
		let id = COUNTER.fetch_add(1, Relaxed);

		let return_chunk = Arc::new(Self {
			id,
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
				let _ = chunk.generate_chunk_data(data);
			}
		});

		return_chunk
	}

	fn generate_chunk_data<'a>(
		&'a self,
		mut data: RwLockWriteGuard<'a, Option<Data>>,
	) -> RwLockReadGuard<'a, Option<Data>> {
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

	pub fn generate_collision_mesh(self: Arc<Self>) {
		rayon::spawn(move || {
			// If try_unwrap returns Ok then nothing else wanted the chunk, so to avoid doing work that will be
			// immidately discarded, we only generate the chunk's collision mesh if we cannot take exclusive ownership of it.
			if let Err(chunk) = Arc::try_unwrap(self) {
				let mut collision = chunk.collision.blocking_write();

				if collision.is_some() {
					return;
				}

				let sector = chunk
					.sector
					.upgrade()
					.expect("Chunk should not be used after Sector has been dropped");

				let chunks = [
					chunk.clone(),
					sector.get_chunk(chunk.coordinates + vector![0, 0, 1]),
					sector.get_chunk(chunk.coordinates + vector![0, 1, 0]),
					sector.get_chunk(chunk.coordinates + vector![0, 1, 1]),
					sector.get_chunk(chunk.coordinates + vector![1, 0, 0]),
					sector.get_chunk(chunk.coordinates + vector![1, 0, 1]),
					sector.get_chunk(chunk.coordinates + vector![1, 1, 0]),
					sector.get_chunk(chunk.coordinates + vector![1, 1, 1]),
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
			}
		});
	}

	pub fn read_data_immediately(&self) -> DataReadGuard {
		{
			let data = self.data.blocking_read();

			if data.is_some() {
				return RwLockReadGuard::map(data, |v| v.as_ref().unwrap());
			}
		}

		let data = self.generate_chunk_data(self.data.blocking_write());
		RwLockReadGuard::map(data, |v| v.as_ref().unwrap())
	}

	pub fn try_read_data(&self) -> DataTryReadGuard {
		self.data.blocking_read()
	}
}

impl Drop for Chunk {
	fn drop(&mut self) {
		if let Some(sector) = Weak::upgrade(&self.sector) {
			sector.chunks.remove(&self.coordinates);
		}
	}
}

pub type DataTryReadGuard<'a> = RwLockReadGuard<'a, Option<Data>>;
pub type DataReadGuard<'a> = RwLockReadGuard<'a, Data>;

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
	pub fn new(sector: &Arc<Sector>, coordinates: ChunkCoordinates, connection: Arc<Connection>) -> Self {
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
	pub fn new(sector: &Arc<Sector>, coordinates: ChunkCoordinates) -> Self {
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
				sector.ticking_chunks.insert(chunk.id, chunk.clone());
				chunk.clone().generate_collision_mesh();
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
					sector.ticking_chunks.remove(&chunk.id);
				}
			}
		}
	}
}
