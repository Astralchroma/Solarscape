use crate::{connection::Connection, generation::ProtoChunk, player::Player};
use log::{error, warn};
use nalgebra::{vector, zero, Isometry3, Vector3, Vector4};
use solarscape_shared::types::ChunkData;
use std::collections::{HashMap, HashSet};
use std::{array, cell::Cell, cell::RefCell, sync::Arc, thread, time::Duration, time::Instant};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};

pub struct Sector {
	voxjects: Box<[Voxject]>,
	players: RefCell<HashMap<Arc<str>, Player>>,
}

impl Sector {
	#[must_use]
	pub fn load() -> Self {
		let (completed_chunks_sender, completed_chunks) = channel();

		Self {
			players: RefCell::new(HashMap::new()),
			voxjects: Box::new([Voxject {
				name: Box::from("example_voxject"),
				location: Cell::new(Isometry3::default()),
				completed_chunks: RefCell::new(completed_chunks),
				completed_chunks_sender,
				pending_chunk_locks: RefCell::new(HashMap::new()),
				chunks: RefCell::new(array::from_fn(|_| HashMap::new())),
			}]),
		}
	}

	pub fn run(self, mut incoming_connections: Receiver<Connection>) {
		let target_tick_time = Duration::from_secs(1) / 30;

		loop {
			let tick_start = Instant::now();

			// Accept one connection, any other connections will simply be handled on the next tick
			match incoming_connections.try_recv() {
				Ok(connection) => {
					let name = connection.name().clone();
					self.players
						.borrow_mut()
						.insert(name, Player::accept(connection, &self));
				}
				Err(TryRecvError::Disconnected) => {
					error!("Connection Channel was dropped!");
					return self.stop();
				}
				_ => {}
			}

			// Process all players
			let mut disconnected_players = HashSet::new();
			for (player_name, player) in self.players.borrow().iter() {
				if player.process_player(&self) {
					disconnected_players.insert(player_name.clone());
				}
			}

			for player_name in disconnected_players {
				if let Some(player) = self.players.borrow_mut().remove(&player_name) {
					for (voxject, levels) in player.loaded_chunks.take().iter().enumerate() {
						let voxject = &self.voxjects[voxject];

						for (level, chunks) in levels.iter().enumerate() {
							for chunk in chunks {
								voxject.release_chunk(&player_name, level, *chunk);
							}
						}
					}
				}
			}

			// Tick Voxjects
			for voxject in self.voxjects.iter() {
				voxject.tick(&self)
			}

			let tick_end = Instant::now();
			let tick_duration = tick_end - tick_start;

			match target_tick_time.checked_sub(tick_duration) {
				Some(time_until_next_tick) => thread::sleep(time_until_next_tick),
				None => warn!("Tick took {tick_duration:.0?}, exceeding {target_tick_time:.0?} target"),
			}
		}
	}

	fn stop(self) {
		drop(self);
	}

	pub const fn voxjects(&self) -> &[Voxject] {
		&self.voxjects
	}
}

pub struct Voxject {
	name: Box<str>,
	pub location: Cell<Isometry3<f32>>,

	completed_chunks: RefCell<Receiver<Chunk>>,
	completed_chunks_sender: Sender<Chunk>,
	pending_chunk_locks: RefCell<HashMap<Vector4<i32>, Vec<Arc<str>>>>,

	chunks: RefCell<[HashMap<Vector3<i32>, Chunk>; 31]>,
}

impl Voxject {
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

			let coordinates = vector![
				chunk.coordinates.x,
				chunk.coordinates.y,
				chunk.coordinates.z,
				chunk.level as i32
			];
			if let Some(chunk_locks) = pending_chunk_locks.remove(&coordinates) {
				for player_name in chunk_locks {
					if let Some(player) = sector_players.get(&player_name) {
						player.on_lock_chunk(&chunk);
						chunk.locks.insert(player_name.clone());
					}
				}

				chunks[chunk.level as usize].insert(chunk.coordinates, chunk);
			}
		}
	}

	pub fn lock_and_load_chunk(
		&self,
		sector: &Sector,
		player_name: &Arc<str>,
		level: usize,
		level_coordinates: Vector3<i32>,
	) {
		let mut pending_chunk_locks = self.pending_chunk_locks.borrow_mut();
		let mut chunks = self.chunks.borrow_mut();
		let coordinates = vector![
			level_coordinates.x,
			level_coordinates.y,
			level_coordinates.z,
			level as i32
		];

		match pending_chunk_locks.get_mut(&coordinates) {
			Some(pending_chunk_lock) => pending_chunk_lock.push(player_name.clone()),
			None => {
				match chunks[level].get_mut(&level_coordinates) {
					Some(chunk) => {
						let player = &sector.players.borrow()[player_name];
						player.on_lock_chunk(chunk);
						chunk.locks.insert(player_name.clone());
					}
					None => {
						let completed_chunks_sender = self.completed_chunks_sender.clone();

						// fifo because unimportant
						rayon::spawn_fifo(move || {
							let chunk = ProtoChunk::new(level as u8, level_coordinates).distance(zero()).build();

							// If this fails then the server is probably shutting down, or is crashing, so it won't matter
							let _ = completed_chunks_sender.send(chunk);
						});

						pending_chunk_locks.insert(coordinates, vec![player_name.clone()]);
					}
				}
			}
		};
	}

	pub fn release_chunk(&self, player_name: &Arc<str>, level: usize, level_coordinates: Vector3<i32>) {
		let level_chunks = &mut self.chunks.borrow_mut()[level];
		if let Some(chunk) = level_chunks.get_mut(&level_coordinates) {
			if chunk.locks.contains(player_name) {
				chunk.locks.remove(player_name);
			}

			if chunk.locks.is_empty() {
				level_chunks.remove(&level_coordinates);
			}
		}
	}

	#[must_use]
	pub const fn name(&self) -> &str {
		&self.name
	}
}

pub struct Chunk {
	pub level: u8,
	pub coordinates: Vector3<i32>,
	pub data: ChunkData,
	pub locks: HashSet<Arc<str>>,
}
