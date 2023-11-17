use crate::connection::ServerConnection;
use hecs::{Component, Entity};
use solarscape_shared::component::{Object, Sector};
use solarscape_shared::protocol::{encode, Message, SyncEntity};

pub type Subscribers = Vec<Entity>;

pub trait Syncable: Component {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection);
}

// Oh look, IntelliJ is hallucinating errors again!
// Component is dynamically implemented by hecs for anything that is Send + Sync + 'static
// So this error claiming that Component is not implemented is nonsense
// - Ferra @ Astralchroma
impl Syncable for Sector {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection) {
		connection.send(encode(Message::SyncEntity {
			entity,
			sync: SyncEntity::Sector(self.clone()),
		}))
	}
}

impl Syncable for Object {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection) {
		connection.send(encode(Message::SyncEntity {
			entity,
			sync: SyncEntity::Object(*self),
		}))
	}
}
