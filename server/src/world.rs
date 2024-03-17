use crate::{connection::Connection, connection::Event, player::Player};
use log::{error, warn};
use nalgebra::{convert, convert_unchecked, Isometry3, Vector3};
use solarscape_shared::messages::clientbound::{AddVoxject, SyncChunk, VoxjectPosition};
use solarscape_shared::messages::serverbound::ServerboundMessage;
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
			voxjects: Box::new([Voxject {
				name: String::from("example_voxject"),
				position: Isometry3::default(),
			}]),
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
					for (index, voxject) in self.voxjects.iter().enumerate() {
						connection.send(AddVoxject {
							id: index,
							name: voxject.name.clone(),
						});
						connection.send(VoxjectPosition {
							id: index,
							position: voxject.position,
						});
					}

					self.players.push(Player {
						connection,
						position: Isometry3::default(),
					});
				}
			}

			self.players.retain_mut(|player| {
				let mut chunks_to_send = vec![];

				for message in player.connection.recv() {
					match message {
						Event::Closed => return false,
						Event::Message(message) => match message {
							ServerboundMessage::PlayerPosition(position) => {
								// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
								player.position = position;

								// This mess can probably be improved
								for (index, voxject) in self.voxjects.iter().enumerate() {
									// These values are local to the level they are on
									// so a 0.5, 0.5, 0.5 player position on level 0 means in chunk 0, 0, 0
									// on the next level that becomes 0.25, 0.25, 0.25 in chunk 0, 0, 0
									// this shit is cursed and hard to understand, but it works!
									let mut player_position: Vector3<f32> =
										voxject.position.inverse_transform_vector(&position.translation.vector) / 16.0;
									let mut player_chunk: Vector3<i32> = convert_unchecked(player_position);
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
													let chunk = Vector3::new(x, y, z);

													// circles look nicer
													let chunk_center =
														convert::<_, Vector3<f32>>(chunk) + Vector3::new(0.5, 0.5, 0.5);
													if player_chunk != chunk
														&& player_position.metric_distance(&chunk_center) as i32
															> chunk_radius
													{
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

										player_position /= 2.0;
										player_chunk.apply(|value| *value >>= 1);

										for chunk in chunks {
											chunks_to_send.push(SyncChunk {
												voxject_id: index,
												level: level as u8,
												grid_coordinate: chunk,
											});
										}

										chunks = next_chunks;
										next_chunks = HashSet::new();
									}
								}
							}
						},
					}
				}

				for chunk in chunks_to_send {
					player.connection.send(chunk);
				}

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
	position: Isometry3<f32>,
}
