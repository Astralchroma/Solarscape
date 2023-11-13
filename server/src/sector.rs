use crate::{connection::ServerConnection, sync::Syncable};
use hecs::Entity;
use serde::Deserialize;
use solarscape_shared::protocol::{encode, Message, SyncEntity};

pub struct Sector {
	pub name: Box<str>,
	pub display_name: Box<str>,
}

#[derive(Deserialize)]
struct SectorConfig {
	pub display_name: Box<str>,
}

impl Syncable for Sector {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection) {
		connection.send(encode(Message::SyncEntity {
			entity,
			sync: SyncEntity::Sector {
				name: self.name.clone(),
				display_name: self.display_name.clone(),
			},
		}))
	}
}
