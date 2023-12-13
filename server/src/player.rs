use crate::voxel_object::calculate_chunk_location;
use crate::{connection::ServerConnection, generator::ArcGenerator, sync, sync::Subscribers};
use hecs::World;
use log::info;
use nalgebra::{convert_unchecked, Vector3};
use solarscape_shared::{chunk::Chunk, components::Location, components::VoxelObject};

pub fn update_position(
	world: &mut World,
	connection_id: usize,
	connection: &ServerConnection,
	position: &Vector3<f64>,
) {
	let mut new_chunks = vec![];

	for (voxject_ent, (_, voxject_loc, voxject_gen)) in &mut world.query::<(&VoxelObject, &Location, &ArcGenerator)>() {
		let player_chunk_grid_pos: Vector3<i32> = convert_unchecked((voxject_loc.position - position) / 16.0);

		for x in player_chunk_grid_pos.x - 5..player_chunk_grid_pos.x + 5 {
			for y in player_chunk_grid_pos.y - 2..player_chunk_grid_pos.y + 5 {
				for z in player_chunk_grid_pos.z - 5..player_chunk_grid_pos.z + 5 {
					let chunk_grid_pos = Vector3::new(x, y, z);

					// TODO: This is probably terrible for performance, we shouldn't iterate over everything just to find one
					//       specific entity
					if let Some((chunk_ent, _)) = world
						.query::<&Chunk>()
						.into_iter()
						.find(|(_, other)| other.voxel_object == voxject_ent && other.grid_position == chunk_grid_pos)
					{
						let _ = sync::subscribe(world, &chunk_ent, connection_id, connection);
						continue;
					}

					let chunk = voxject_gen.generate_chunk(voxject_ent, 0, chunk_grid_pos);
					let chunk_loc = calculate_chunk_location(voxject_loc, &chunk);

					info!(
						"Generated chunk {:?} at {chunk_loc:?} in voxel object located at {voxject_loc:?}",
						chunk.grid_position
					);

					new_chunks.push((chunk, chunk_loc, Subscribers::new()));
				}
			}
		}
	}

	for chunk in new_chunks {
		let chunk_ent = world.spawn(chunk);

		let _ = sync::subscribe(world, &chunk_ent, connection_id, connection);
	}
}
