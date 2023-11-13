use crate::{connection::ServerConnection, generator::Generator, sync::Subscribers, sync::Syncable};
use hecs::QueryOneError::NoSuchEntity;
use hecs::{Entity, QueryOneError, World};
use nalgebra::Vector3;
use solarscape_shared::protocol::{encode, Message, SyncEntity};

pub const CHUNK_RADIUS: i32 = 3;

// TODO: anything higher than 8 causes overflow, look into this later
pub const OCTREE_LEVELS: u8 = 8;

pub struct Object {
	pub sector: Entity,

	pub generator: Box<dyn Generator + Send + Sync>,
}

impl Object {
	/// TODO: Temporary
	pub fn generate_sphere(world: &mut World, object_entity: Entity) -> Result<(), QueryOneError> {
		let mut query = world.query_one::<&Object>(object_entity)?;
		let object = query.get().ok_or(NoSuchEntity)?;

		let mut chunks = vec![];

		for level in 0..OCTREE_LEVELS {
			for x in -CHUNK_RADIUS..CHUNK_RADIUS {
				for y in -CHUNK_RADIUS..CHUNK_RADIUS {
					for z in -CHUNK_RADIUS..CHUNK_RADIUS {
						chunks.push((
							object
								.generator
								.generate_chunk(object_entity, level, Vector3::new(x, y, z)),
							Subscribers::new(),
						));
					}
				}
			}
		}

		drop(query);

		world.spawn_batch(chunks);

		Ok(())
	}
}

impl Syncable for Object {
	fn sync(&self, entity: Entity, connection: &mut ServerConnection) {
		connection.send(encode(Message::SyncEntity {
			entity,
			sync: SyncEntity::Object,
		}))
	}
}
