use crate::sync::{Subscribers, Syncable};
use crate::{chunk::Chunk, connection::ServerConnection};
use hecs::{Entity, World};
use log::warn;
use solarscape_shared::component::{Object, Sector};
use solarscape_shared::{protocol::encode, protocol::Event, protocol::Message, TICK_DURATION};
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
					let mut connection = self
						.world
						.get::<&mut ServerConnection>(entity)
						.expect("spawned connection");

					let mut query = self
						.world
						.query_one::<(&Sector, &mut Subscribers)>(self.default_sector)
						.expect("default sector");
					let (sector, sector_subscribers) = query.get().expect("default sector");

					sector_subscribers.push(entity);
					sector.sync(self.default_sector, &mut connection);

					connection.send(encode(Message::Event(Event::ActiveSector(self.default_sector))));

					self.world
						.query::<(&Object, &mut Subscribers)>()
						.into_iter()
						.filter(|(_, (object, _))| object.sector == self.default_sector)
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
