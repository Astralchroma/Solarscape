use crate::{generator::BoxedGenerator, sync::Subscribers};
use hecs::QueryOneError::NoSuchEntity;
use hecs::{Entity, QueryOneError, World};
use nalgebra::Vector3;

// TODO: anything higher than 8 causes overflow, look into this later
pub const OCTREE_LEVELS: u8 = 8;

// Temporary
pub const CHUNK_RADIUS: i32 = 3;

// Temporary
pub fn generate_sphere(world: &mut World, entity: Entity) -> Result<(), QueryOneError> {
	let mut query = world.query_one::<&BoxedGenerator>(entity)?;
	let generator = query.get().ok_or(NoSuchEntity)?;

	let mut chunks = vec![];

	for level in 0..OCTREE_LEVELS {
		for x in -CHUNK_RADIUS..CHUNK_RADIUS {
			for y in -CHUNK_RADIUS..CHUNK_RADIUS {
				for z in -CHUNK_RADIUS..CHUNK_RADIUS {
					chunks.push((
						generator.generate_chunk(entity, level, Vector3::new(x, y, z)),
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
