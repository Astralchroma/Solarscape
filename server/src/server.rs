use crate::{connection::ServerConnection, sync::subscribe};
use hecs::{Entity, World};
use log::warn;
use solarscape_shared::{components::VoxelObject, TICK_DURATION};
use std::{thread, time::Instant};
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};

pub struct Server {
	pub default_sector: Entity,

	pub world: World,
}

impl Server {
	pub fn run(mut self, mut incoming_connections: UnboundedReceiver<ServerConnection>) -> ! {
		loop {
			let tick_start = Instant::now();

			self.process_incoming_connections(&mut incoming_connections);
			self.remove_dead_connections();

			let tick_end = Instant::now();
			let tick_time = tick_end - tick_start;
			match TICK_DURATION.checked_sub(tick_time) {
				Some(duration) => thread::sleep(duration),
				None => warn!("tick took too long! {tick_time:#?}"),
			}
		}
	}

	fn process_incoming_connections(&mut self, incoming_connections: &mut UnboundedReceiver<ServerConnection>) {
		loop {
			match incoming_connections.try_recv() {
				Err(ref error) => match error {
					TryRecvError::Empty => return, // No more incoming connections, we're done here
					TryRecvError::Disconnected => todo!("handle loss of listener"),
				},
				Ok(connection) => {
					// TODO: This seems like it could be a heck of a lot better
					// TODO: We should be defining the initial sector in the server, instead of just picking the first
					let entity = self.world.spawn((connection,));

					subscribe(self, &self.default_sector, &entity).expect("TODO: Error Handling");

					for (voxel_object_entity, voxel_object) in &mut self.world.query::<&VoxelObject>() {
						if voxel_object.sector != self.default_sector {
							continue;
						}

						subscribe(self, &voxel_object_entity, &entity).expect("TODO: Error Handling");
					}
				}
			}
		}
	}

	fn remove_dead_connections(&mut self) {
		let mut dead_connections = vec![];
		for (entity, connection) in self.world.query::<&ServerConnection>().into_iter() {
			if !connection.is_alive() {
				dead_connections.push(entity);
			}
		}
		for entity in dead_connections {
			let _ = self.world.despawn(entity);
		}
	}
}
