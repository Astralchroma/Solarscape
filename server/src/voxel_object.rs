use crate::{connection::ServerConnection, generator::BoxedGenerator, sync::Subscribers};
use hecs::{Entity, NoSuchEntity, QueryOneError, World};
use nalgebra::Vector3;
use solarscape_shared::protocol::{encode, Message, SyncEntity};
use solarscape_shared::{chunk::Chunk, components::Location};

// TODO: anything higher than 8 causes overflow, look into this later
pub const OCTREE_LEVELS: u8 = 1;

// Temporary
pub const CHUNK_RADIUS: i32 = 3;

// Temporary
pub fn generate_sphere(world: &mut World, voxel_object_entity: Entity) -> Result<(), QueryOneError> {
	let mut query = world.query_one::<(&Location, &BoxedGenerator)>(voxel_object_entity)?;
	let (voxel_object_location, generator) = query.get().ok_or(NoSuchEntity)?;

	let mut chunks = vec![];

	for level in 0..OCTREE_LEVELS {
		for x in -CHUNK_RADIUS..CHUNK_RADIUS {
			for y in -CHUNK_RADIUS..CHUNK_RADIUS {
				for z in -CHUNK_RADIUS..CHUNK_RADIUS {
					let chunk = generator.generate_chunk(voxel_object_entity, level, Vector3::new(x, y, z));

					chunks.push((
						calculate_chunk_location(voxel_object_location, &chunk),
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
		position: object_location.position + (chunk.grid_position * (16 * chunk.octree_node.scale() as i32)).cast(),
		rotation: object_location.rotation,
		scale: chunk.octree_node.scale() as f32,
	}
}

// POV: You wrote an entire function only to realise you don't actually need it yet.
/// Updates the locations of all chunks belonging to a VoxelObject. Typically used when the position of the VoxelObject
/// changes.
pub fn update_chunk_locations(world: &mut World, voxel_object_entity: Entity) -> Result<(), QueryOneError> {
	let mut voxel_object_location_query = world.query_one::<&Location>(voxel_object_entity)?;
	let object_location = voxel_object_location_query.get().ok_or(NoSuchEntity)?;

	for (chunk_entity, (chunk, location, subscribers)) in world.query::<(&Chunk, &mut Location, &Subscribers)>().iter()
	{
		if chunk.voxel_object != voxel_object_entity {
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
