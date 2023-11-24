use crate::{chunk::Chunk, connection::ServerConnection, generator::BoxedGenerator, sync::Subscribers};
use hecs::{Entity, NoSuchEntity, QueryOneError, World};
use nalgebra::Vector3;
use solarscape_shared::{component::Location, protocol::encode, protocol::Message, protocol::SyncEntity};

// TODO: anything higher than 8 causes overflow, look into this later
pub const OCTREE_LEVELS: u8 = 8;

// Temporary
pub const CHUNK_RADIUS: i32 = 3;

// Temporary
pub fn generate_sphere(world: &mut World, object_entity: Entity) -> Result<(), QueryOneError> {
	let mut query = world.query_one::<(&Location, &BoxedGenerator)>(object_entity)?;
	let (object_location, generator) = query.get().ok_or(NoSuchEntity)?;

	let mut chunks = vec![];

	for level in 0..OCTREE_LEVELS {
		for x in -CHUNK_RADIUS..CHUNK_RADIUS {
			for y in -CHUNK_RADIUS..CHUNK_RADIUS {
				for z in -CHUNK_RADIUS..CHUNK_RADIUS {
					let chunk = generator.generate_chunk(object_entity, level, Vector3::new(x, y, z));

					chunks.push((
						calculate_chunk_location(object_location, &chunk),
						chunk,
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

#[must_use]
pub fn calculate_chunk_location(object_location: &Location, chunk: &Chunk) -> Location {
	Location {
		position: object_location.position + (chunk.grid_position * (16 * chunk.chunk_type.scale() as i32)).cast(),
		rotation: object_location.rotation,
		scale: chunk.chunk_type.scale() as f32,
	}
}

// POV: You wrote an entire function only to realise you don't actually need it yet.
/// Updates the locations of all chunks belonging to an Object. Typically used when the position of the object changes.
pub fn update_chunk_locations(world: &mut World, object_entity: Entity) -> Result<(), QueryOneError> {
	let mut object_location_query = world.query_one::<&Location>(object_entity)?;
	let object_location = object_location_query.get().ok_or(NoSuchEntity)?;

	for (chunk_entity, (chunk, location, subscribers)) in world.query::<(&Chunk, &mut Location, &Subscribers)>().iter()
	{
		if chunk.object != object_entity {
			continue;
		}

		*location = calculate_chunk_location(object_location, chunk);

		let packet = encode(Message::SyncEntity {
			entity: chunk_entity,
			sync: SyncEntity::Location(*location),
		});

		for connection in subscribers {
			world
				.query_one::<&ServerConnection>(*connection)?
				.get()
				.ok_or(NoSuchEntity)?
				.send(packet.clone());
		}
	}

	Ok(())
}
