use crate::{connection::Connection, connection::Event, generation::ProtoChunk, world::Sector};
use nalgebra::{convert, convert_unchecked, zero, Isometry3, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, SyncChunk, SyncVoxject};
use solarscape_shared::messages::serverbound::ServerboundMessage;
use std::{array, cell::RefCell, collections::HashSet, mem};

pub struct Player {
	connection: RefCell<Connection>,

	location: Isometry3<f32>,

	chunk_list: Vec<[HashSet<Vector3<i32>>; 31]>,
}

impl Player {
	pub fn accept(connection: Connection, sector: &Sector) -> Self {
		for (voxject_index, voxject) in sector.voxjects.iter().enumerate() {
			connection.send(AddVoxject { voxject_index, name: Box::from(voxject.name()) });
			connection.send(SyncVoxject { voxject_index, location: *voxject.location() });
		}

		Self { connection: RefCell::new(connection), location: Default::default(), chunk_list: vec![] }
	}

	pub fn process_player(&mut self, sector: &Sector) -> bool {
		loop {
			let message = match self.connection.borrow_mut().recv() {
				None => break,
				Some(message) => message,
			};

			match message {
				Event::Closed => return false,
				Event::Message(message) => match message {
					ServerboundMessage::PlayerLocation(location) => {
						// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
						self.location = location;

						self.refresh_chunks(sector);

						self.chunk_list.iter().enumerate().for_each(|(voxject_index, levels)| {
							levels.iter().enumerate().for_each(|(level, chunks)| {
								let level = level as u8;

								chunks.iter().for_each(|&coordinates| {
									let data = ProtoChunk::new(level, coordinates).distance(zero()).build().data;
									self.connection.borrow_mut().send(SyncChunk {
										voxject_index,
										level,
										coordinates,
										data,
									})
								})
							})
						});
					}
				},
			}
		}

		true
	}

	pub fn refresh_chunks(&mut self, sector: &Sector) {
		let mut new_chunk_list = vec![];

		for voxject in sector.voxjects.iter() {
			let mut voxject_chunk_list = array::from_fn(|_| HashSet::new());

			// These values are local to the level they are on. So a 0.5, 0.5, 0.5 player position on level 0 means in
			// chunk 0, 0, 0 on the next level that becomes 0.25, 0.25, 0.25 in chunk 0, 0, 0.
			let mut p_pos = voxject
				.location()
				.inverse_transform_vector(&self.location.translation.vector)
				/ 16.0;
			let mut p_chunk: Vector3<i32> = convert_unchecked(p_pos);
			let mut chunks: HashSet<Vector3<i32>> = HashSet::new();
			let mut next_chunks = HashSet::new();

			for level in 0..31 {
				let l_radius = ((level + 1) * 2) >> level;

				for chunk in &chunks {
					next_chunks.insert(chunk.map(|value| value >> 1));
				}

				for x in p_chunk.x - l_radius..=p_chunk.x + l_radius {
					for y in p_chunk.y - l_radius..=p_chunk.y + l_radius {
						for z in p_chunk.z - l_radius..=p_chunk.z + l_radius {
							let chunk = Vector3::new(x, y, z);

							// circles look nicer
							let c_center = convert::<_, Vector3<f32>>(chunk) + Vector3::repeat(0.5);

							if p_chunk != chunk && p_pos.metric_distance(&c_center) as i32 > l_radius {
								continue;
							}

							next_chunks.insert(chunk.map(|value| value >> 1));
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

			new_chunk_list.push(voxject_chunk_list);
		}

		self.chunk_list = new_chunk_list;
	}
}
