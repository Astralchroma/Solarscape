use crate::sync::{Subscribers, Syncable};
use crate::{chunk::Chunk, connection::ServerConnection, object::Object, sector::Sector};
use hecs::World;
use solarscape_shared::protocol::{encode, Event, Message};
use std::{thread, time::Duration, time::Instant};
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};

const TICKS_PER_SECOND: u32 = 30;

#[derive(Default)]
pub struct Server {
	pub world: World,
}

impl Server {
	pub fn run(mut self, mut incoming_connections: UnboundedReceiver<ServerConnection>) -> ! {
		let tick_time_target = Duration::from_secs(1) / TICKS_PER_SECOND;

		loop {
			let tick_start = Instant::now();

			self.process_incoming_connections(&mut incoming_connections);
			self.remove_dead_connections();

			let tick_end = Instant::now();
			let tick_time = tick_end - tick_start;
			thread::sleep(tick_time_target - tick_time);
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
					let mut connection = self
						.world
						.get::<&mut ServerConnection>(entity)
						.expect("spawned connection");

					let mut query = self.world.query::<(&Sector, &mut Subscribers)>();
					let (sector_entity, (sector, subscribers)) = query.iter().next().expect("a sector");

					subscribers.push(entity);
					sector.sync(sector_entity, &mut connection);

					connection.send(encode(Message::Event(Event::ActiveSector(sector_entity))));

					self.world
						.query::<(&Object, &mut Subscribers)>()
						.into_iter()
						.filter(|(_, (object, _))| object.sector == sector_entity)
						.map(|(object_entity, (object, subscribers))| {
							subscribers.push(entity);
							object.sync(object_entity, &mut connection);

							object_entity
						})
						.collect::<Vec<_>>()
						.into_iter()
						.for_each(|object_entity| {
							self.world
								.query::<(&Chunk, &mut Subscribers)>()
								.into_iter()
								.filter(|(_, (chunk, _))| chunk.object == object_entity)
								.for_each(|(chunk_entity, (chunk, subscribers))| {
									subscribers.push(entity);
									chunk.sync(chunk_entity, &mut connection);
								});
						});
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
