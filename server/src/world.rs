use crate::{generation::sphere_generator, generation::Generator, player::ConnectingPlayer, player::Player};
use log::{error, warn};
use nalgebra::Isometry3;
use solarscape_shared::{messages::clientbound::SyncChunk, types::ChunkData, types::GridCoordinates};
use std::collections::{HashMap, HashSet};
use std::{cell::Cell, cell::RefCell, ops::Deref, ops::DerefMut, sync::Arc, thread, time::Duration, time::Instant};
use thiserror::Error;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};

pub struct Sector {
	voxjects: Box<[Voxject]>,
	players: RefCell<HashMap<Arc<str>, Player>>,

	connecting_players: Receiver<ConnectingPlayer>,
}

impl Sector {
	pub const fn voxjects(&self) -> &[Voxject] {
		&self.voxjects
	}

	#[must_use]
	pub fn load(connecting_players: Receiver<ConnectingPlayer>) -> Self {
		let (completed_chunks_sender, completed_chunks) = channel();

		Self {
			players: RefCell::new(HashMap::new()),
			voxjects: Box::new([Voxject {
				_name: Box::from("example_voxject"),
				location: Cell::new(Isometry3::default()),
				generator: Generator::new(&sphere_generator),
				completed_chunks: RefCell::new(completed_chunks),
				completed_chunks_sender,
				pending_chunk_locks: RefCell::new(HashMap::new()),
				chunks: RefCell::new(HashMap::new()),
			}]),

			connecting_players,
		}
	}

	pub fn run(mut self) {
		let target_tick_time = Duration::from_secs(1) / 30;

		loop {
			let tick_start = Instant::now();

			if let Err(error) = self.tick() {
				error!("Fatal error occurred while ticking sector, stopping.\n{error}");
				self.stop();
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

	fn tick(&mut self) -> Result<(), SectorTickError> {
		{
			let mut players = self.players.borrow_mut();

			// Remove disconnected players
			let disconnected_players = players
				.iter()
				.filter(|(_, player)| player.is_disconnected())
				.map(|(name, _)| name.clone())
				.collect::<Vec<_>>();

			for player_name in disconnected_players {
				if let Some(player) = players.remove(&player_name) {
					for (voxject, levels) in player.loaded_chunks.take().iter().enumerate() {
						let voxject = &self.voxjects[voxject];

						for coordinates in levels {
							voxject.release_chunk(&player_name, coordinates);
						}
					}
				}
			}
		}

		// Handle connecting players
		loop {
			match self.connecting_players.try_recv() {
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => return Err(SectorTickError::Dropped),
				Ok(connecting_player) => {
					let player = Player::accept(connecting_player);
					self.players.borrow_mut().insert(player.name().clone(), player);
				}
			}
		}

		// Process all players
		for player in self.players.borrow().values() {
			player.process_player(self)
		}

		// Tick Voxjects
		for voxject in self.voxjects.iter() {
			voxject.tick(self)
		}

		Ok(())
	}

	fn stop(self) {
		drop(self);
	}
}

#[derive(Debug, Error)]
enum SectorTickError {
	#[error("sector handle was dropped")]
	Dropped,
}

pub struct Voxject {
	_name: Box<str>,
	pub location: Cell<Isometry3<f32>>,

	generator: Generator,

	completed_chunks: RefCell<Receiver<Chunk>>,
	completed_chunks_sender: Sender<Chunk>,
	pending_chunk_locks: RefCell<HashMap<GridCoordinates, Vec<Arc<str>>>>,

	chunks: RefCell<HashMap<GridCoordinates, Chunk>>,
}

impl Voxject {
	#[must_use]
	pub const fn _name(&self) -> &str {
		&self._name
	}

	fn tick(&self, sector: &Sector) {
		// Handle completed chunks
		let mut completed_chunks = self.completed_chunks.borrow_mut();
		let mut pending_chunk_locks = self.pending_chunk_locks.borrow_mut();
		let sector_players = sector.players.borrow();
		let mut chunks = self.chunks.borrow_mut();

		loop {
			let mut chunk = match completed_chunks.try_recv() {
				Err(TryRecvError::Disconnected) => unreachable!(),
				Err(TryRecvError::Empty) => break,
				Ok(chunk) => chunk,
			};

			if let Some(chunk_locks) = pending_chunk_locks.remove(&chunk.coordinates) {
				for player_name in chunk_locks {
					if let Some(player) = sector_players.get(&player_name) {
						player.send(SyncChunk { voxject: 0, data: chunk.data.clone() });
						chunk.locks.insert(player_name.clone());
					}
				}

				chunks.insert(chunk.coordinates, chunk);
			}
		}
	}

	pub fn lock_and_load_chunk(&self, sector: &Sector, player_name: &Arc<str>, coordinates: GridCoordinates) {
		let mut pending_chunk_locks = self.pending_chunk_locks.borrow_mut();
		let mut chunks = self.chunks.borrow_mut();

		match pending_chunk_locks.get_mut(&coordinates) {
			Some(pending_chunk_lock) => pending_chunk_lock.push(player_name.clone()),
			None => match chunks.get_mut(&coordinates) {
				Some(chunk) => {
					let player = &sector.players.borrow()[player_name];
					player.send(SyncChunk { voxject: 0, data: chunk.data.clone() });
					chunk.locks.insert(player_name.clone());
				}
				None => {
					self.generator.generate(coordinates, &self.completed_chunks_sender);
					pending_chunk_locks.insert(coordinates, vec![player_name.clone()]);
				}
			},
		};
	}

	pub fn release_chunk(&self, player_name: &Arc<str>, coordinates: &GridCoordinates) {
		let chunks = &mut self.chunks.borrow_mut();
		if let Some(chunk) = chunks.get_mut(coordinates) {
			if chunk.locks.contains(player_name) {
				chunk.locks.remove(player_name);
			}

			if chunk.locks.is_empty() {
				chunks.remove(coordinates);
			}
		}
	}
}

#[must_use]
pub struct Chunk {
	pub data: ChunkData,
	pub locks: HashSet<Arc<str>>,
}

impl Chunk {
	pub fn new(grid_coordinates: GridCoordinates) -> Self {
		Self { data: ChunkData::from(grid_coordinates), locks: HashSet::new() }
	}
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
