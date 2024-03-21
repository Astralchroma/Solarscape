use crate::{connection::Connection, player::Player};
use log::{error, warn};
use nalgebra::{Isometry3, Vector3};
use solarscape_shared::{messages::clientbound::AddVoxject, messages::clientbound::SyncVoxject, types::ChunkData};
use std::{thread, time::Duration, time::Instant};
use tokio::{runtime::Handle, sync::mpsc::error::TryRecvError, sync::mpsc::Receiver};

pub struct World {
	_runtime: Handle,
	incoming_connections: Receiver<Connection>,
	pub voxjects: Box<[Voxject]>,
}

impl World {
	#[must_use]
	pub fn load(runtime: Handle, incoming_connections: Receiver<Connection>) -> Self {
		Self {
			_runtime: runtime,
			incoming_connections,
			voxjects: Box::new([Voxject { name: Box::from("example_voxject"), location: Isometry3::default() }]),
		}
	}

	pub fn run(mut self) {
		let mut players = vec![];

		let target_tick_time = Duration::from_secs(1) / 30;
		// let mut last_tick_start = Instant::now();
		loop {
			let tick_start = Instant::now();
			// let tick_delta = tick_start - last_tick_start;
			// last_tick_start = tick_start;

			// only accept one connection, we'll handle the rest on the next tick anyway
			match self.incoming_connections.try_recv() {
				Err(error) => {
					if error == TryRecvError::Disconnected {
						error!("Connection Channel was dropped!");
						return self.stop();
					}
				}
				Ok(connection) => players.push(Player::accept(connection, &self)),
			}

			players.retain_mut(|player| player.process_player(&self));

			let tick_end = Instant::now();
			let tick_duration = tick_end - tick_start;
			if let Some(time_until_next_tick) = target_tick_time.checked_sub(tick_duration) {
				thread::sleep(time_until_next_tick);
			} else {
				warn!("Tick took {tick_duration:.0?}, exceeding {target_tick_time:.0?} target");
			}
		}
	}

	fn stop(self) {
		drop(self);
	}
}

pub struct Voxject {
	name: Box<str>,
	location: Isometry3<f32>,
}

impl Voxject {
	#[must_use]
	pub const fn name(&self) -> &str {
		&self.name
	}

	#[must_use]
	pub const fn location(&self) -> &Isometry3<f32> {
		&self.location
	}
}

pub struct Chunk {
	pub level: u8,
	pub coordinates: Vector3<i32>,
	pub data: ChunkData,
}
