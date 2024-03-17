use crate::{connection::Connection, connection::Event, generation::ProtoChunk, player::Player};
use log::{error, warn};
use nalgebra::{convert, convert_unchecked, zero, Isometry3, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, SyncChunk, SyncVoxject};
use solarscape_shared::{messages::serverbound::ServerboundMessage, types::ChunkData};
use std::{collections::HashSet, thread, time::Duration, time::Instant};
use tokio::{runtime::Handle, sync::mpsc::error::TryRecvError, sync::mpsc::Receiver};

pub struct World {
	_runtime: Handle,
	incoming_connections: Receiver<Connection>,
	players: Vec<Player>,
	voxjects: Box<[Voxject]>,
}

impl World {
	#[must_use]
	pub fn load(runtime: Handle, incoming_connections: Receiver<Connection>) -> Self {
		Self {
			_runtime: runtime,
			incoming_connections,
			players: vec![],
			voxjects: Box::new([Voxject { name: String::from("example_voxject"), location: Isometry3::default() }]),
		}
	}

	pub fn run(mut self) {
		let target_tick_time = Duration::from_secs(1) / 30;
		// let mut last_tick_start = Instant::now();
		loop {
			let tick_start = Instant::now();
			// let tick_delta = tick_start - last_tick_start;
			// last_tick_start = tick_start;

			match self.incoming_connections.try_recv() {
				Err(error) => {
					if error == TryRecvError::Disconnected {
						error!("Connection Channel was dropped!");
						return self.stop();
					}
				}
				Ok(connection) => {
					for (voxject_index, voxject) in self.voxjects.iter().enumerate() {
						connection.send(AddVoxject { voxject_index, name: voxject.name.clone() });
						connection.send(SyncVoxject { voxject_index, location: voxject.location });
					}

					self.players.push(Player { connection, location: Isometry3::default() });
				}
			}

			self.players.retain_mut(|player| {
				let mut chunks_to_send = vec![];

				for message in player.connection.recv() {
					match message {
						Event::Closed => return false,
						Event::Message(message) => match message {
							ServerboundMessage::PlayerLocation(location) => {
								// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
								player.location = location;

								// This mess can probably be improved
								for (index, voxject) in self.voxjects.iter().enumerate() {
									// These values are local to the level they are on
									// so a 0.5, 0.5, 0.5 player position on level 0 means in chunk 0, 0, 0
									// on the next level that becomes 0.25, 0.25, 0.25 in chunk 0, 0, 0
									// this shit is cursed and hard to understand, but it works!
									let mut player_location: Vector3<f32> =
										voxject.location.inverse_transform_vector(&location.translation.vector) / 16.0;
									let mut player_chunk: Vector3<i32> = convert_unchecked(player_location);
									let mut chunks: HashSet<Vector3<i32>> = HashSet::new();
									let mut next_chunks = HashSet::new();

									for level in 0..31 {
										let chunk_radius = ((level + 1) * 2) >> level;

										for chunk in &chunks {
											next_chunks.insert(chunk.map(|value| value >> 1));
										}

										for x in player_chunk.x - chunk_radius..=player_chunk.x + chunk_radius {
											for y in player_chunk.y - chunk_radius..=player_chunk.y + chunk_radius {
												for z in player_chunk.z - chunk_radius..=player_chunk.z + chunk_radius {
													let coordinates = Vector3::new(x, y, z);

													// circles look nicer
													let center = convert::<_, Vector3<f32>>(coordinates)
														+ Vector3::new(0.5, 0.5, 0.5);
													if player_chunk != coordinates
														&& player_location.metric_distance(&center) as i32
															> chunk_radius
													{
														continue;
													}

													next_chunks.insert(coordinates.map(|value| value >> 1));
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

										player_location /= 2.0;
										player_chunk.apply(|value| *value >>= 1);

										for chunk in chunks {
											chunks_to_send.push((index, level, chunk));
										}

										chunks = next_chunks;
										next_chunks = HashSet::new();
									}
								}
							}
						},
					}
				}

				chunks_to_send
					.into_iter()
					.map(|(voxject_index, level, coordinates)| {
						(
							voxject_index,
							ProtoChunk::new(level as u8, coordinates)
								.distance(zero())
								.set_greater_than(1000.0, 0.0)
								.build(),
						)
					})
					.map(|(voxject_index, chunk)| SyncChunk {
						voxject_index,
						level: chunk.level,
						coordinates: chunk.coordinates,
						data: chunk.data,
					})
					.for_each(|sync_chunk| player.connection.send(sync_chunk));

				true
			});

			let tick_end = Instant::now();
			let tick_duration = tick_end - tick_start;
			if let Some(time_until_next_tick) = target_tick_time.checked_sub(tick_duration) {
				thread::sleep(time_until_next_tick);
			} else {
				warn!("Tick took {tick_duration:?}, exceeding {target_tick_time:?} target");
			}
		}
	}

	fn stop(self) {
		drop(self);
	}
}

pub struct Voxject {
	name: String,
	location: Isometry3<f32>,
}

pub struct Chunk {
	pub level: u8,
	pub coordinates: Vector3<i32>,
	pub data: ChunkData,
}
