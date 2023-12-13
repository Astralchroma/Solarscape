use crate::{connection::ServerConnection, player, sync::subscribe};
use hecs::{Entity, World};
use log::warn;
use solarscape_shared::protocol::{DisconnectReason, Event, Message};
use solarscape_shared::{components::VoxelObject, TICK_DURATION};
use std::{collections::HashMap, thread, time::Instant};
use tokio::{runtime::Runtime, sync::mpsc::error::TryRecvError, sync::mpsc::UnboundedReceiver};

pub struct Server {
	pub runtime: Runtime,
	pub default_sector: Entity,

	pub world: World,

	pub next_connection_id: usize,
	pub connections: HashMap<usize, ServerConnection>,
}

impl Server {
	pub fn run(mut self, mut incoming_connections: UnboundedReceiver<ServerConnection>) -> ! {
		loop {
			let tick_start = Instant::now();

			self.process_incoming_connections(&mut incoming_connections);
			self.handle_incoming_messages();
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
					let connection_id = self.next_connection_id;
					self.next_connection_id += 1;
					self.connections.insert(connection_id, connection);
					let connection = self
						.connections
						.get(&connection_id)
						.expect("connection should not be removed right after we added it");

					subscribe(&self.world, &self.default_sector, connection_id, connection)
						.expect("TODO: Error Handling");

					for (voxel_object_entity, voxel_object) in &mut self.world.query::<&VoxelObject>() {
						if voxel_object.sector != self.default_sector {
							continue;
						}

						subscribe(&self.world, &voxel_object_entity, connection_id, connection)
							.expect("TODO: Error Handling");
					}
				}
			}
		}
	}

	fn remove_dead_connections(&mut self) {
		// TODO: Unsubscribe from everything
		self.connections.retain(|_, connection| connection.is_alive());
	}

	fn handle_incoming_messages(&mut self) {
		for (connection_id, connection) in &mut self.connections {
			while let Ok(message) = connection.receive().try_recv() {
				match message {
					Message::SyncEntity { .. } => connection.disconnect(DisconnectReason::ProtocolViolation),
					Message::Event(event) => match event {
						Event::PositionUpdated(player_pos) => {
							player::update_position(&mut self.world, *connection_id, connection, &player_pos)
						}
						_ => connection.disconnect(DisconnectReason::ProtocolViolation),
					},
				}
			}
		}
	}
}
