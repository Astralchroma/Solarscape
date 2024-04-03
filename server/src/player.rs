use crate::{connection::Connection, connection::Event, world::Chunk, world::Sector};
use nalgebra::{convert, convert_unchecked, Isometry3, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, SyncChunk, SyncVoxject};
use solarscape_shared::messages::serverbound::ServerboundMessage;
use std::{array, cell::Cell, cell::RefCell, collections::HashSet, iter::repeat, iter::zip, mem};

pub struct Player {
	connection: RefCell<Connection>,
	location: Cell<Isometry3<f32>>,

	pub loaded_chunks: RefCell<Box<[[HashSet<Vector3<i32>>; 31]]>>,
}

impl Player {
	pub fn accept(connection: Connection, sector: &Sector) -> Self {
		for (voxject_index, voxject) in sector.voxjects().iter().enumerate() {
			connection.send(AddVoxject { voxject_index, name: Box::from(voxject.name()) });
			connection.send(SyncVoxject { voxject_index, location: voxject.location.get() });
		}

		Self {
			connection: RefCell::new(connection),
			location: Default::default(),
			loaded_chunks: RefCell::new(
				repeat(array::from_fn(|_| HashSet::new()))
					.take(sector.voxjects().len())
					.collect(),
			),
		}
	}

	/// Called by the `Voxject` when a `Chunk` is locked by the `Player`. If the chunk is loaded, this is called
	/// immediately after the `Player` locks the chunk, otherwise it is called once the chunk is loaded.
	pub fn on_lock_chunk(&self, chunk: &Chunk) {
		self.connection.borrow().send(SyncChunk {
			voxject_index: 0,
			level: chunk.level,
			coordinates: chunk.coordinates,
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

						for (voxject, (new_levels, old_levels)) in
							zip(new_chunk_list.iter(), old_chunk_list.iter()).enumerate()
						{
							for (level, (new_chunks, old_chunks)) in
								zip(new_levels.iter(), old_levels.iter()).enumerate()
							{
								let added_chunks = new_chunks.difference(old_chunks);

								for chunk in added_chunks {
									sector.voxjects()[voxject].lock_and_load_chunk(
										sector,
										self.connection.borrow().name(),
										level,
										*chunk,
									);
								}

								let removed_chunks = old_chunks.difference(new_chunks);

								for chunk in removed_chunks {
									sector.voxjects()[voxject].release_chunk(
										self.connection.borrow().name(),
										level,
										*chunk,
									);
								}
							}
						}
					}
				},
			}
		}
	}

	pub fn generate_chunk_list(&self, sector: &Sector) -> Box<[[HashSet<Vector3<i32>>; 31]]> {
		let mut chunk_list = vec![];

		for voxject in sector.voxjects().iter() {
			let mut voxject_chunk_list = array::from_fn(|_| HashSet::new());

			// These values are local to the level they are on. So a 0.5, 0.5, 0.5 player position on level 0 means in
			// chunk 0, 0, 0 on the next level that becomes 0.25, 0.25, 0.25 in chunk 0, 0, 0.
			let mut p_pos = voxject
				.location
				.get()
				.inverse_transform_vector(&self.location.get().translation.vector)
				/ 16.0;
			let mut p_chunk: Vector3<i32> = convert_unchecked(p_pos);
			let mut chunks: HashSet<Vector3<i32>> = HashSet::new();
			let mut next_chunks = HashSet::new();

			for level in 0..31 {
				let l_radius = ((level + 1) * 2) >> level;

				for chunk in &chunks {
					next_chunks.insert(chunk.map(|value| value >> 1));
				}

				for c_x in p_chunk.x - l_radius..=p_chunk.x + l_radius {
					for c_y in p_chunk.y - l_radius..=p_chunk.y + l_radius {
						for c_z in p_chunk.z - l_radius..=p_chunk.z + l_radius {
							let c_chunk = Vector3::new(c_x, c_y, c_z);

							// circles look nicer
							let c_center = convert::<_, Vector3<f32>>(c_chunk) + Vector3::repeat(0.5);

							if p_chunk != c_chunk && p_pos.metric_distance(&c_center) as i32 > l_radius {
								continue;
							}

							next_chunks.insert(c_chunk.map(|value| value >> 1));
						}
					}
				}

				for upleveled_chunk in &next_chunks {
					let chunk = upleveled_chunk.map(|value| value << 1);
					chunks.insert(chunk + Vector3::new(0, 0, 0));
					chunks.insert(chunk + Vector3::new(0, 0, 1));
					chunks.insert(chunk + Vector3::new(0, 1, 0));
					chunks.insert(chunk + Vector3::new(0, 1, 1));
					chunks.insert(chunk + Vector3::new(1, 0, 0));
					chunks.insert(chunk + Vector3::new(1, 0, 1));
					chunks.insert(chunk + Vector3::new(1, 1, 0));
					chunks.insert(chunk + Vector3::new(1, 1, 1));
				}

				p_pos /= 2.0;
				p_chunk.apply(|value| *value >>= 1);

				mem::swap(&mut chunks, &mut next_chunks);
				mem::swap(&mut next_chunks, &mut voxject_chunk_list[level as usize]);
			}

			chunk_list.push(voxject_chunk_list);
		}

		chunk_list.into_boxed_slice()
	}
}
