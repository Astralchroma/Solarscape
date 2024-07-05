use crate::{config, generation::sphere_generator, generation::Generator, player::Connection, player::Player};
use dashmap::DashMap;
use log::{error, warn};
use solarscape_shared::messages::clientbound::{ClientboundMessage, SyncChunk};
use solarscape_shared::types::{ChunkCoordinates, Material, VoxjectId};
use std::sync::{atomic::AtomicUsize, atomic::Ordering::Relaxed, Arc, Weak};
use std::{collections::HashMap, ops::Deref, thread, time::Duration, time::Instant};
use thiserror::Error;
use tokio::sync::RwLockReadGuard;
use tokio::sync::{mpsc::error::TryRecvError, mpsc::UnboundedReceiver as Receiver, Mutex, RwLock};

pub struct Sector {
	pub name: Box<str>,

	voxjects: HashMap<VoxjectId, Voxject>,
	chunks: DashMap<ChunkCoordinates, Weak<Chunk>>,

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
		let voxject = Self { id, name, generator: sphere_generator };
		(id, voxject)
	}
}

#[non_exhaustive]
pub struct Chunk {
	pub sector: Weak<Sector>,
	pub coordinates: ChunkCoordinates,

	// This is deliberately a `Mutex<HashMap<K, V>>` instead of a `DashMap<K, V>` as when locking the chunk for a
	// connection, we need to prevent another thread from syncing at the same time, otherwise it could cause a desync.
	subscribers: Mutex<HashMap<usize, Arc<Connection>>>,

	data: RwLock<Option<Data>>,
}

impl Chunk {
	fn new(sector: &Arc<Sector>, coordinates: ChunkCoordinates) -> Arc<Self> {
		let return_chunk = Arc::new(Self {
			sector: Arc::downgrade(sector),
			coordinates,
			subscribers: Mutex::new(HashMap::new()),
			data: RwLock::default(),
		});

		let generator = sector.voxjects[&coordinates.voxject].generator;
		let chunk = return_chunk.clone();
		rayon::spawn(move || {
			if let Err(chunk) = Arc::try_unwrap(chunk) {
				let mut data = chunk.data.blocking_write();

				if data.is_none() {
					*data = Some(generator(&chunk.coordinates));
				}

				let data = data.downgrade();
				let subscribers = chunk.subscribers.blocking_lock();

				let message = ClientboundMessage::SyncChunk(SyncChunk {
					coordinates,
					materials: data.as_ref().unwrap().materials.clone(),
					densities: data.as_ref().unwrap().densities.clone(),
				});

				for connection in subscribers.values() {
					connection.send(message.clone())
				}
			}
		});

		return_chunk
	}

	pub fn try_read_data(&self) -> TryDataReadGuard {
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

pub type TryDataReadGuard<'a> = RwLockReadGuard<'a, Option<Data>>;

pub struct Data {
	pub materials: Box<[Material; 4096]>,
	pub densities: Box<[f32; 4096]>,
}

impl Default for Data {
	fn default() -> Self {
		Self { materials: Box::new([Material::Nothing; 4096]), densities: Box::new([0.0; 4096]) }
	}
}

pub struct ClientLock {
	chunk: Arc<Chunk>,
	id: usize,
}

impl ClientLock {
	pub fn new(chunk: Arc<Chunk>, connection: Arc<Connection>) -> Self {
		static COUNTER: AtomicUsize = AtomicUsize::new(0);
		let id = COUNTER.fetch_add(1, Relaxed);

		{
			let mut subscribers = chunk.subscribers.blocking_lock();

			if let Some(ref data) = *chunk.try_read_data() {
				connection.send(SyncChunk {
					coordinates: chunk.coordinates,
					materials: data.materials.clone(),
					densities: data.densities.clone(),
				});
			}

			subscribers.insert(id, connection);
		}

		Self { chunk, id }
	}
}

impl Deref for ClientLock {
	type Target = Chunk;

	fn deref(&self) -> &Self::Target {
		&self.chunk
	}
}

impl Drop for ClientLock {
	fn drop(&mut self) {
		self.chunk.subscribers.blocking_lock().remove(&self.id);
	}
}
