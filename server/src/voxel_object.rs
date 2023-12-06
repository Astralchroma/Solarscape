use crate::{server::Server, sync::Subscribers};
use hecs::{Entity, NoSuchEntity, QueryOneError};
use solarscape_shared::protocol::{encode, Message, SyncEntity};
use solarscape_shared::{chunk::Chunk, components::Location};

#[must_use]
pub fn calculate_chunk_location(object_location: &Location, chunk: &Chunk) -> Location {
	Location {
		position: object_location.position + chunk.voxel_object_relative_position().cast(),
		rotation: object_location.rotation,
		scale: (chunk.level + 1) as f32,
	}
}

// POV: You wrote an entire function only to realise you don't actually need it yet.
/// Updates the locations of all chunks belonging to a VoxelObject. Typically used when the position of the VoxelObject
/// changes.
pub fn update_chunk_locations(server: &mut Server, voxel_object_entity: Entity) -> Result<(), QueryOneError> {
	let mut voxel_object_location_query = server.world.query_one::<&Location>(voxel_object_entity)?;
	let object_location = voxel_object_location_query.get().ok_or(NoSuchEntity)?;

	for (chunk_entity, (chunk, location, subscribers)) in
		server.world.query::<(&Chunk, &mut Location, &Subscribers)>().iter()
	{
		if chunk.voxel_object != voxel_object_entity {
			continue;
		}

		*location = calculate_chunk_location(object_location, chunk);

		let packet = encode(Message::SyncEntity {
			entity: chunk_entity,
			sync: SyncEntity::Location(*location),
		});

		for connection_id in subscribers {
			if let Some(connection) = server.connections.get(connection_id) {
				connection.send(packet.clone());
			}
		}
	}

	Ok(())
}
