use crate::sync::{Subscribers, Syncable};
use crate::{chunk::Chunk, connection::Connection};
use hecs::{Entity, World};
use nalgebra::Vector3;
use solarscape_shared::protocol::Clientbound;

pub const CHUNK_RADIUS: i32 = 1;
pub const RADIUS: f64 = (CHUNK_RADIUS << 4) as f64 - 0.5;

pub struct Object {
	pub sector: Entity,
}

impl Object {
	/// TODO: Temporary
	pub fn generate_sphere(world: &mut World, object: Entity) {
		for x in -CHUNK_RADIUS..CHUNK_RADIUS {
			for y in -CHUNK_RADIUS..CHUNK_RADIUS {
				for z in -CHUNK_RADIUS..CHUNK_RADIUS {
					let chunk_grid_position = Vector3::new(x, y, z);
					let mut chunk = Chunk::new(object, chunk_grid_position);
					chunk.generate_sphere_section();
					world.spawn((chunk, Subscribers::new()));
				}
			}
		}
	}
}

impl Syncable for Object {
	fn sync(&self, entity: Entity, connection: &mut Connection) {
		let object_id = entity.to_bits().get();
		connection.send(Clientbound::AddObject { entity_id: object_id });
	}
}
