use crate::{world::Sector, Sectors};
use axum::extract::ws::{close_code, CloseFrame, Message, WebSocket};
use axum::extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade};
use axum::{http::StatusCode, response::IntoResponse, response::Response};
use log::{error, info, warn};
use nalgebra::{convert_unchecked, vector, Isometry3, Vector3};
use serde::Deserialize;
use solarscape_shared::messages::clientbound::{AddVoxject, ClientboundMessage, RemoveChunk, SyncVoxject};
use solarscape_shared::{messages::serverbound::ServerboundMessage, types::GridCoordinates};
use std::cell::{Cell, OnceCell, RefCell};
use std::{borrow::Cow, collections::HashSet, iter::repeat, iter::zip, net::SocketAddr, sync::Arc};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use tokio::sync::oneshot::{channel as oneshot, Receiver as OneshotReceiver, Sender as OneshotSender};
use tokio::{pin, select, sync::mpsc::error::SendError, sync::RwLock, time::interval, time::Duration, time::Instant};

pub struct Player {
	name: Arc<str>,

	location: Cell<Isometry3<f32>>,
	pub loaded_chunks: RefCell<Box<[HashSet<GridCoordinates>]>>,

	_latency: Arc<RwLock<Duration>>,

	disconnect: OnceCell<OneshotSender<Option<CloseFrame<'static>>>>,
	incoming: RefCell<Receiver<ServerboundMessage>>,
	outgoing: Sender<ClientboundMessage>,
}

pub struct ConnectingPlayer {
	name: Arc<str>,

	socket: WebSocket,
}

#[derive(Deserialize)]
pub struct NameQuery {
	name: Arc<str>,
}

impl Player {
	#[must_use]
	pub const fn name(&self) -> &Arc<str> {
		&self.name
	}

	#[must_use]
	pub fn _latency(&self) -> Duration {
		*self._latency.blocking_read()
	}

	pub fn is_disconnected(&self) -> bool {
		self.disconnect.get().is_none()
	}

	pub fn send(&self, message: impl Into<ClientboundMessage>) {
		let _ = self.outgoing.send(message.into());
	}

	pub async fn connect(
		ConnectInfo(address): ConnectInfo<SocketAddr>,
		State(sectors): State<Sectors>,
		Path(sector): Path<String>,
		Query(NameQuery { name }): Query<NameQuery>,
		socket: WebSocketUpgrade,
	) -> Response {
		let invalid_name = || -> Response {
			warn!("[{address}] Failed to connect due to invalid name!");
			(StatusCode::BAD_REQUEST, "Name must match `[-.\\w]{3,32}`").into_response()
		};

		// These name checks are pretty simple check to implement, no need to pull in an entire regex library for it
		if !(3..=32).contains(&name.len()) {
			return invalid_name();
		}
		for character in name.chars() {
			match character {
				'-' | '.' | '0'..='9' | 'A'..='Z' | '_' | 'a'..='z' => continue,
				_ => return invalid_name(),
			}
		}

		sectors.get(&sector).map_or_else(
			|| StatusCode::NOT_FOUND.into_response(),
			|sector_handle| {
				let sector_handle = sector_handle.clone();
				socket.on_upgrade(|socket| async move {
					info!("[{name}] Connecting to {sector:?}");

					let player = ConnectingPlayer { name: name.clone(), socket };

					if sector_handle.send(player).is_err() {
						warn!("[{name}] Failed to connect to {sector:?}, sector handle has been dropped.")
					}
				})
			},
		)
	}

	pub fn accept(sector: &Sector, connecting_player: ConnectingPlayer) -> Self {
		let ConnectingPlayer { name, socket } = connecting_player;

		let latency: Arc<RwLock<_>> = Default::default();
		let (disconnect, handler_disconnect) = oneshot();
		let (handler_incoming, incoming) = channel();
		let (outgoing, handler_outgoing) = channel();

		tokio::spawn(Self::manage_connection_handler(
			name.clone(),
			latency.clone(),
			handler_disconnect,
			handler_incoming,
			handler_outgoing,
			socket,
		));

		let connection = Self {
			name,
			location: Default::default(),
			loaded_chunks: RefCell::new(repeat(HashSet::new()).take(sector.voxjects().len()).collect()),
			_latency: latency,
			disconnect: OnceCell::from(disconnect),
			incoming: RefCell::new(incoming),
			outgoing,
		};

		for (voxject_index, voxject) in sector.voxjects().iter().enumerate() {
			connection.send(AddVoxject { voxject: voxject_index, name: voxject.name().into() });
			connection.send(SyncVoxject { voxject: voxject_index, location: voxject.location.get() });
		}

		connection
	}

	async fn manage_connection_handler(
		name: Arc<str>,
		latency: Arc<RwLock<Duration>>,
		disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		incoming: Sender<ServerboundMessage>,
		outgoing: Receiver<ClientboundMessage>,
		mut socket: WebSocket,
	) {
		info!("[{name}] Connected");

		let reason = Self::connection_handler(latency, disconnect, incoming, outgoing, &mut socket)
			.await
			.unwrap_or_else(|error| match error {
				Error::Dropped => {
					warn!("[{name}] Connection was dropped unexpectedly!");
					Closed::Server(Some(CloseFrame {
						code: close_code::ERROR,
						reason: Cow::Borrowed("Internal Error"),
					}))
				}
				Error::Invalid => Closed::Server(Some(CloseFrame {
					code: close_code::INVALID,
					reason: Cow::Borrowed("Invalid Data"),
				})),
				Error::TimedOut => Closed::Server(Some(CloseFrame {
					code: close_code::PROTOCOL,
					reason: Cow::Borrowed("Timed Out"),
				})),
				Error::Unknown(error) => {
					error!("[{name}] Error in connection handler: {error}");
					Closed::Server(Some(CloseFrame {
						code: close_code::ERROR,
						reason: Cow::Borrowed("Internal Error"),
					}))
				}
			});

		match reason {
			Closed::Client(frame) => {
				let frame = frame
					.as_ref()
					.unwrap_or(&CloseFrame { code: close_code::ABNORMAL, reason: Cow::Borrowed("Abnormal") });
				info!("[{name}] Disconnected by Client: {} {}", frame.code, frame.reason);
			}
			Closed::Server(frame) => {
				{
					let frame = frame
						.as_ref()
						.unwrap_or(&CloseFrame { code: close_code::ERROR, reason: Cow::Borrowed("Unknown") });
					info!("[{name}] Disconnected by Server: {} {}", frame.code, frame.reason);
				}
				let _ = socket.send(Message::Close(frame)).await;
			}
		};
	}

	async fn connection_handler(
		latency: Arc<RwLock<Duration>>,
		mut disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		incoming: Sender<ServerboundMessage>,
		mut outgoing: Receiver<ClientboundMessage>,
		socket: &mut WebSocket,
	) -> Result<Closed, Error> {
		let mut last_pings: [Duration; 12] = [Duration::default(); 12];
		let mut pending_pong: Option<([u8; 32], Instant)> = None;
		let ping_interval = interval(Duration::from_secs(5));
		pin!(ping_interval);

		loop {
			select! {
				biased;
				reason = &mut disconnect => {
					let reason = reason.unwrap_or(None);
					return Ok(Closed::Server(reason));
				}
				_ = ping_interval.tick() => {
					match pending_pong {
						None => {
							let ping = rand::random();
							pending_pong = Some((ping, Instant::now()));
							socket.send(Message::Ping(Vec::from(&ping))).await?;
						}
						Some(_) => return Err(Error::TimedOut),
					}
				}
				message = outgoing.recv() => {
					let message = message.ok_or(Error::Dropped)?;
					let bytes = bincode::serialize(&message).map_err(|error| Error::Unknown(error.into()))?;
					socket.send(Message::Binary(bytes)).await?;
				}
				message = socket.recv() => {
					let message = match message {
						None => return Ok(Closed::Client(None)),
						Some(message) => message,
					};

					match message? {
						Message::Text(_) => return Err(Error::Invalid),
						Message::Binary(data) => {
							let message = bincode::deserialize(&data).map_err(|_| Error::Invalid)?;
							incoming.send(message)?;
						},
						Message::Ping(data) => socket.send(Message::Pong(data)).await?,
						Message::Pong(pong) => {
							match pending_pong {
								None => return Err(Error::Invalid),
								Some((expected_pong, time)) => {
									if pong != expected_pong {
										return Err(Error::Invalid);
									}

									let round_trip_time = Instant::now() - time;
									last_pings.copy_within(1.., 0);
									last_pings[11] = round_trip_time;

									*latency.write().await = last_pings.iter()
										.fold(Duration::ZERO, |total, ping| total + *ping)
										/ 12;

									pending_pong = None;
								}
							}
						}
						Message::Close(reason) => return Ok(Closed::Client(reason)),
					}
				}
			}
		}
	}

	pub fn process_player(&self, sector: &Sector) {
		while let Ok(message) = self.incoming.borrow_mut().try_recv() {
			match message {
				ServerboundMessage::PlayerLocation(location) => {
					// TODO: Check that this makes sense, we don't want players to just teleport :foxple:
					self.location.set(location);

					let old = self.loaded_chunks.replace(self.generate_chunk_list(sector));
					let new = self.loaded_chunks.borrow();

					for (voxject, (new, old)) in zip(new.iter(), old.iter()).enumerate() {
						let added_chunks = new.difference(old);

						for coordinates in added_chunks {
							sector.voxjects()[voxject].lock_and_load_chunk(sector, self.name(), *coordinates);
						}

						let removed_chunks = old.difference(new);

						for coordinates in removed_chunks {
							sector.voxjects()[voxject].release_chunk(self.name(), coordinates);
							self.send(RemoveChunk { voxject, coordinates: *coordinates })
						}
					}
				}
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

enum Closed {
	Client(Option<CloseFrame<'static>>),
	Server(Option<CloseFrame<'static>>),
}

enum Error {
	Dropped,
	Invalid,
	TimedOut,
	Unknown(anyhow::Error),
}

impl From<axum::Error> for Error {
	fn from(error: axum::Error) -> Self {
		Self::Unknown(error.into())
	}
}

impl<T> From<SendError<T>> for Error {
	fn from(_: SendError<T>) -> Self {
		Self::Dropped
	}
}
