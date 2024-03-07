use crate::connection::Connection;
use log::error;
use nalgebra::Isometry3;
use solarscape_shared::messages::clientbound::AddVoxject;
use std::{mem, thread, time::Duration, time::Instant};
use tokio::{runtime::Handle, sync::mpsc::error::TryRecvError, sync::mpsc::Receiver};

pub struct World {
	_runtime: Handle,
	incoming_connections: Receiver<Connection>,
	voxjects: Box<[Voxject]>,
}

impl World {
	pub fn load(runtime: Handle, incoming_connections: Receiver<Connection>) -> Self {
		Self {
			_runtime: runtime,
			incoming_connections,
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
					}

					mem::forget(connection); // temp
				}
			}

			let tick_end = Instant::now();
			let tick_duration = tick_end - tick_start;
			thread::sleep(target_tick_time - tick_duration);
		}
	}

	pub fn stop(self) {
		drop(self);
	}
}

pub struct Voxject {
	pub name: String,
	pub position: Isometry3<f32>,
}
