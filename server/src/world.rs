use crate::{connection::Connection, connection::Event, player::Player};
use log::error;
use nalgebra::Isometry3;
use solarscape_shared::messages::clientbound::{AddVoxject, VoxjectPosition};
use solarscape_shared::messages::serverbound::ServerboundMessage;
use std::{thread, time::Duration, time::Instant};
use tokio::{runtime::Handle, sync::mpsc::error::TryRecvError, sync::mpsc::Receiver};

pub struct World {
	_runtime: Handle,
	incoming_connections: Receiver<Connection>,
	players: Vec<Player>,
	voxjects: Box<[Voxject]>,
}

impl World {
	pub fn load(runtime: Handle, incoming_connections: Receiver<Connection>) -> Self {
		Self {
			_runtime: runtime,
			incoming_connections,
			players: vec![],
			voxjects: Box::new([Voxject {
				name: String::from("example_voxject"),
				position: Isometry3::default(),
			}]),
		}
	}

	pub fn run(mut self) {
		let target_tick_time = Duration::from_secs(1) / 30;
		// let mut last_tick_start = Instant::now();
		loop {
			let tick_start = Instant::now();
			// let tick_delta = tick_start - last_tick_start;
			// last_tick_start = tick_start;

			match self.incoming_connections.try_recv() {
				Err(error) => {
					if error == TryRecvError::Disconnected {
						error!("Connection Channel was dropped!");
						return self.stop();
					}
				}
				Ok(connection) => {
					for (index, voxject) in self.voxjects.iter().enumerate() {
						connection.send(AddVoxject {
							id: index,
							name: voxject.name.clone(),
						});
						connection.send(VoxjectPosition {
							id: index,
							position: voxject.position,
						});
					}

					self.players.push(Player {
						connection,
						position: Isometry3::default(),
					});
				}
			}

			self.players.retain_mut(|player| {
				for message in player.connection.recv() {
					match message {
						Event::Closed => return false,
						Event::Message(message) => match message {
							// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
							ServerboundMessage::PlayerPosition(position) => player.position = position,
						},
					}
				}

				true
			});

			let tick_end = Instant::now();
			let tick_duration = tick_end - tick_start;
			thread::sleep(target_tick_time - tick_duration);
		}
	}

	fn stop(self) {
		drop(self);
	}
}

pub struct Voxject {
	name: String,
	position: Isometry3<f32>,
}
