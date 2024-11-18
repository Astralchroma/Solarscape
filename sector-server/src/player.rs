use crate::sector::{ClientLock, Sector, SharedSector, TickLock};
use nalgebra::{convert_unchecked, vector, IsometryMatrix3, Vector3};
use solarscape_shared::{
	connection::{Connection, ServerEnd},
	data::{
		world::{ChunkCoordinates, Level, Location, LEVELS},
		Id,
	},
	message::clientbound::{InventorySlot, Sync, Voxject},
};
use sqlx::{query_as, PgPool};
use std::{
	collections::HashSet,
	ops::{Deref, DerefMut},
	sync::Arc,
};
use tokio::runtime::Handle;

pub struct Player {
	pub id: Id,
	pub connection: Connection<ServerEnd>,

	pub location: Location,

	pub client_locks: Vec<ClientLock>,
	pub tick_locks: Vec<TickLock>,
}

impl Player {
	pub fn accept(sector: &Sector, id: Id, connection: Connection<ServerEnd>) -> Self {
		connection.send(Sync {
			name: sector.name.clone(),

			voxjects: sector
				.voxjects
				.iter()
				.map(|(id, voxject)| Voxject {
					id: *id,
					name: voxject.name.clone(),
				})
				.collect(),
			structures: sector
				.structures
				.iter()
				.map(|structure| structure.build_sync(&sector.physics))
				.collect(),

			inventory: Self::get_inventory(id, &sector.database),
		});

		Self {
			id,
			connection,
			location: Location::default(),
			client_locks: vec![],
			tick_locks: vec![],
		}
	}

	pub fn get_inventory(id: Id, database: &PgPool) -> Vec<InventorySlot> {
		Handle::current()
			.block_on(
				query_as!(
					InventorySlot,
					r#"SELECT item AS "item: Item", COUNT(*) as "quantity!"
						FROM items JOIN inventory_items ON id = item_id
						WHERE inventory_id = $1
						GROUP BY item"#,
					id as _,
				)
				.fetch_all(database),
			)
			.expect("inventory")
	}

	pub fn compute_locks(
		&self,
		sector: &Arc<SharedSector>,
	) -> (HashSet<ChunkCoordinates>, HashSet<ChunkCoordinates>) {
		const MULTIPLIER: i32 = 1;

		let mut client_locks = HashSet::new();
		let mut tick_locks = HashSet::new();

		for voxject in sector.voxjects.values() {
			// These values are relative to the current level. So a player position of
			// (0.5 0.5 0.5, Chunk 0 0 0, Level 0) is the same as (0.25 0.25 0.25, Chunk 0, 0, 0, Level 1).

			// Voxjects temporarily do not have a position until we intograte Rapier
			let mut player_position = IsometryMatrix3::default()
				.inverse_transform_vector(&self.location.position.coords)
				/ 16.0;
			let mut player_chunk = ChunkCoordinates::new(
				voxject.id,
				convert_unchecked(player_position),
				Level::new(0),
			);
			let mut level_chunks = HashSet::new();

			tick_locks.insert(player_chunk);

			for level in 0..LEVELS - 1 {
				let level = Level::new(level);
				let radius = ((*level as i32 / LEVELS as i32) * MULTIPLIER + MULTIPLIER) >> *level;

				if radius > 0 {
					for x in
						player_chunk.coordinates.x - radius..=player_chunk.coordinates.x + radius
					{
						for y in player_chunk.coordinates.y - radius
							..=player_chunk.coordinates.y + radius
						{
							for z in player_chunk.coordinates.z - radius
								..=player_chunk.coordinates.z + radius
							{
								let chunk =
									ChunkCoordinates::new(voxject.id, vector![x, y, z], level);

								// circles look nicer
								let chunk_center =
									vector![x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5];
								if player_chunk != chunk
									&& player_position.metric_distance(&chunk_center) as i32
										> radius
								{
									continue;
								}

								level_chunks.insert(chunk.upleveled());
							}
						}
					}
				}

				for chunk in &level_chunks {
					let chunk = chunk.downleveled();
					client_locks.insert(chunk + Vector3::new(0, 0, 0));
					client_locks.insert(chunk + Vector3::new(0, 0, 1));
					client_locks.insert(chunk + Vector3::new(0, 1, 0));
					client_locks.insert(chunk + Vector3::new(0, 1, 1));
					client_locks.insert(chunk + Vector3::new(1, 0, 0));
					client_locks.insert(chunk + Vector3::new(1, 0, 1));
					client_locks.insert(chunk + Vector3::new(1, 1, 0));
					client_locks.insert(chunk + Vector3::new(1, 1, 1));
				}

				player_position /= 2.0;
				player_chunk = player_chunk.upleveled();

				if *level < LEVELS - 2 {
					level_chunks = level_chunks
						.into_iter()
						.map(|chunk| chunk.upleveled())
						.collect();
				}
			}
		}

		(client_locks, tick_locks)
	}
}

impl Deref for Player {
	type Target = Connection<ServerEnd>;

	fn deref(&self) -> &Self::Target {
		&self.connection
	}
}

impl DerefMut for Player {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.connection
	}
}
