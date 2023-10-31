use crate::connection::ServerConnection;
use hecs::{Component, Entity};

pub type Subscribers = Vec<Entity>;

pub trait Syncable: Component {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection);
}
