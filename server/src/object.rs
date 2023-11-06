use crate::generator::{Generator, SphereGenerator};
use crate::{connection::ServerConnection, sync::Subscribers, sync::Syncable};
use hecs::{Entity, World};
use nalgebra::Vector3;
use solarscape_shared::protocol::{encode, Message, SyncEntity};

pub const CHUNK_RADIUS: i32 = 3;

pub struct Object {
	pub sector: Entity,
}

impl Object {
	/// TODO: Temporary
	pub fn generate_sphere(world: &mut World, object: Entity) {
		let generator = SphereGenerator {
			radius: (CHUNK_RADIUS << 4) as f32 - 0.5,
		};

		for x in -CHUNK_RADIUS..CHUNK_RADIUS {
			for y in -CHUNK_RADIUS..CHUNK_RADIUS {
				for z in -CHUNK_RADIUS..CHUNK_RADIUS {
					world.spawn((
						generator.generate_chunk(object, Vector3::new(x, y, z)),
						Subscribers::new(),
					));
				}
			}
		}
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
