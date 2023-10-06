use crate::connection::Connection;
use hecs::{Component, Entity};

pub type Subscribers = Vec<Entity>;

pub trait Syncable: Component {
	fn sync(&self, connection: &mut Connection);
}
