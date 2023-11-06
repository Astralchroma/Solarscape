use crate::{chunk::Chunk, connection::ServerConnection, generator::Generator, sync::Subscribers, sync::Syncable};
use hecs::Entity;
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
	pub fn generate_sphere(&self, object: Entity) -> Vec<(Chunk, Vec<Entity>)> {
		let mut chunks = vec![];

		for level in 0..OCTREE_LEVELS {
			for x in -CHUNK_RADIUS..CHUNK_RADIUS {
				for y in -CHUNK_RADIUS..CHUNK_RADIUS {
					for z in -CHUNK_RADIUS..CHUNK_RADIUS {
						chunks.push((
							self.generator.generate_chunk(object, level, Vector3::new(x, y, z)),
							Subscribers::new(),
						));
					}
				}
			}
		}

		chunks
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
