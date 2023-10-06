use crate::sync::{Subscribers, Syncable};
use crate::{connection::Connection, object::Object, sector::Sector};
use hecs::World;
use solarscape_shared::protocol::Clientbound;
use tokio::sync::mpsc::{error::TryRecvError, UnboundedReceiver};

#[derive(Default)]
pub struct Server {
	pub world: World,
}

impl Server {
	pub fn run(mut self, mut incoming_connections: UnboundedReceiver<Connection>) -> ! {
		loop {
			self.process_incoming_connections(&mut incoming_connections);
		}
	}

	fn process_incoming_connections(&mut self, incoming_connections: &mut UnboundedReceiver<Connection>) {
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
					let mut connection = self.world.get::<&mut Connection>(entity).expect("spawned connection");

					let mut query = self.world.query::<(&Sector, &mut Subscribers)>();
					let (sector_entity, (sector, subscribers)) = query.iter().next().expect("a sector");

					subscribers.push(entity);
					sector.sync(&mut connection);

					connection.send(Clientbound::ActiveSector { sector_id: 0 });

					self.world
						.query::<&Object>()
						.iter()
						.map(|(_, object)| object)
						.filter(|object| object.sector == sector_entity)
						.for_each(|object| object.sync(&mut connection));
				}
			}
		}
	}
}
