use crate::{connection::Connection, connection::Event, world::Chunk, world::Sector};
use nalgebra::{convert_unchecked, vector, Isometry3, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, RemoveChunk, SyncChunk, SyncVoxject};
use solarscape_shared::{messages::serverbound::ServerboundMessage, types::GridCoordinates};
use std::{cell::Cell, cell::RefCell, collections::HashSet, iter::repeat, iter::zip};

pub struct Player {
	connection: RefCell<Connection>,
	location: Cell<Isometry3<f32>>,

	pub loaded_chunks: RefCell<Box<[HashSet<GridCoordinates>]>>,
}

impl Player {
	pub fn accept(connection: Connection, sector: &Sector) -> Self {
		for (voxject_index, voxject) in sector.voxjects().iter().enumerate() {
			connection.send(AddVoxject { voxject: voxject_index, name: Box::from(voxject.name()) });
			connection.send(SyncVoxject { voxject: voxject_index, location: voxject.location.get() });
		}

		Self {
			connection: RefCell::new(connection),
			location: Default::default(),
			loaded_chunks: RefCell::new(repeat(HashSet::new()).take(sector.voxjects().len()).collect()),
		}
	}

	/// Called by the `Voxject` when a `Chunk` is locked by the `Player`. If the chunk is loaded, this is called
	/// immediately after the `Player` locks the chunk, otherwise it is called once the chunk is loaded.
	pub fn on_lock_chunk(&self, chunk: &Chunk) {
		self.connection.borrow().send(SyncChunk {
			voxject: 0, // TODO
			data: chunk.data.clone(),
		})
	}

	/// Returns `true` if the connection is closed
	pub fn process_player(&self, sector: &Sector) -> bool {
		loop {
			let message = match self.connection.borrow_mut().recv() {
				None => return false,
				Some(message) => message,
			};

			match message {
				Event::Closed => return true,
				Event::Message(message) => match message {
					ServerboundMessage::PlayerLocation(location) => {
						// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
						self.location.set(location);

						let old_chunk_list = self.loaded_chunks.replace(self.generate_chunk_list(sector));
						let new_chunk_list = self.loaded_chunks.borrow();

						for (voxject, (new_chunks, old_chunks)) in
							zip(new_chunk_list.iter(), old_chunk_list.iter()).enumerate()
						{
							let added_chunks = new_chunks.difference(old_chunks);

							for coordinates in added_chunks {
								sector.voxjects()[voxject].lock_and_load_chunk(
									sector,
									self.connection.borrow().name(),
									*coordinates,
								);
							}

							let removed_chunks = old_chunks.difference(new_chunks);

							for coordinates in removed_chunks {
								sector.voxjects()[voxject].release_chunk(self.connection.borrow().name(), coordinates);
								self.connection
									.borrow()
									.send(RemoveChunk { voxject, coordinates: *coordinates })
							}
						}
					}
				},
			}
		}
	}

	pub fn generate_chunk_list(&self, sector: &Sector) -> Box<[HashSet<GridCoordinates>]> {
		let mut chunk_list: Box<_> = repeat(HashSet::new()).take(sector.voxjects().len()).collect();

		for (voxject, voxject_chunks) in sector.voxjects().iter().zip(chunk_list.iter_mut()) {
			// These values are local to the level they are on. So a 0.5, 0.5, 0.5 player position on level 0 means in
			// chunk 0, 0, 0 on the next level that becomes 0.25, 0.25, 0.25 in chunk 0, 0, 0.
			let mut player_position = voxject
				.location
				.get()
				.inverse_transform_vector(&self.location.get().translation.vector)
				/ 16.0;
			let mut player_chunk = GridCoordinates::new(convert_unchecked(player_position), 0);
			let mut chunks = HashSet::<GridCoordinates>::new();
			let mut upleveled_chunks = HashSet::new();

			for level in 0..31u8 {
				let radius = ((level as i32 + 1) * 2) >> level;

				for chunk in &chunks {
					upleveled_chunks.insert(chunk.upleveled());
				}

				for x in player_chunk.coordinates.x - radius..=player_chunk.coordinates.x + radius {
					for y in player_chunk.coordinates.y - radius..=player_chunk.coordinates.y + radius {
						for z in player_chunk.coordinates.z - radius..=player_chunk.coordinates.z + radius {
							let chunk = GridCoordinates::new(vector![x, y, z], level);

							// circles look nicer
							let chunk_center = vector![x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5];
							if player_chunk != chunk && player_position.metric_distance(&chunk_center) as i32 > radius {
								continue;
							}

							upleveled_chunks.insert(chunk.upleveled());
						}
					}
				}

				for upleveled_chunk in &upleveled_chunks {
					let chunk = upleveled_chunk.downleveled();
					chunks.insert(chunk + Vector3::new(0, 0, 0));
					chunks.insert(chunk + Vector3::new(0, 0, 1));
					chunks.insert(chunk + Vector3::new(0, 1, 0));
					chunks.insert(chunk + Vector3::new(0, 1, 1));
					chunks.insert(chunk + Vector3::new(1, 0, 0));
					chunks.insert(chunk + Vector3::new(1, 0, 1));
					chunks.insert(chunk + Vector3::new(1, 1, 0));
					chunks.insert(chunk + Vector3::new(1, 1, 1));
				}

				player_position /= 2.0;
				player_chunk = player_chunk.upleveled();

				voxject_chunks.extend(chunks);
				chunks = upleveled_chunks;
				upleveled_chunks = HashSet::new();
			}
		}

		chunk_list
	}
}
