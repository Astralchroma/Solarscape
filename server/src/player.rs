use crate::{sector::ClientLock, sector::Event, sector::Sector, sector::SectorHandle, sector::TickLock, Sectors};
use axum::extract::ws::{close_code, CloseFrame, Message, WebSocket};
use axum::extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade};
use axum::{http::StatusCode, response::IntoResponse, response::Response};
use log::{error, info, warn};
use nalgebra::{convert_unchecked, vector, Isometry3, Vector3};
use serde::Deserialize;
use solarscape_shared::messages::clientbound::{AddVoxject, ClientboundMessage, SyncVoxject};
use solarscape_shared::{messages::serverbound::ServerboundMessage, types::ChunkCoordinates, types::Level};
use std::sync::{atomic::AtomicUsize, atomic::Ordering::Relaxed, Arc};
use std::{borrow::Cow, collections::HashSet, net::SocketAddr, ops::Deref};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use tokio::sync::oneshot::{channel as oneshot, Receiver as OneshotReceiver, Sender as OneshotSender};
use tokio::sync::{mpsc::error::SendError, mpsc::error::TryRecvError, Mutex, RwLock};
use tokio::{pin, select, time::interval, time::Duration, time::Instant};

pub struct Player {
	pub connection: Arc<Connection>,

	pub location: Isometry3<f32>,

	pub client_locks: Vec<ClientLock>,
	pub tick_locks: Vec<TickLock>,
}

impl Player {
	pub fn accept(sector: &Sector, connection: Arc<Connection>) -> Self {
		for (id, voxject) in &sector.voxjects {
			connection.send(AddVoxject {
				id: *id,
				name: voxject.name.clone(),
			});
			connection.send(SyncVoxject {
				id: *id,
				location: Isometry3::default(),
			});
		}

		Self {
			connection,
			location: Isometry3::default(),
			client_locks: vec![],
			tick_locks: vec![],
		}
	}

	pub fn compute_locks(&self, sector: &Arc<SectorHandle>) -> (HashSet<ChunkCoordinates>, Vec<ChunkCoordinates>) {
		const MULTIPLIER: i32 = 1;

		let mut client_locks = HashSet::new();
		let mut tick_locks = Vec::new();

		for voxject in sector.voxjects.values() {
			// These values are relative to the current level. So a player position of
			// (0.5 0.5 0.5, Chunk 0 0 0, Level 0) is the same as (0.25 0.25 0.25, Chunk 0, 0, 0, Level 1).

			// Voxjects temporarily do not have a position until we intograte Rapier
			let mut player_position =
				Isometry3::default().inverse_transform_vector(&self.location.translation.vector) / 16.0;
			let mut player_chunk = ChunkCoordinates::new(voxject.id, convert_unchecked(player_position), Level::new(0));
			let mut level_chunks = HashSet::new();

			tick_locks.push(player_chunk);

			for level in 0..31u8 {
				let level = Level::new(level);
				let radius = ((*level as i32 / 32) * MULTIPLIER + MULTIPLIER) >> *level;

				if radius > 0 {
					for x in player_chunk.coordinates.x - radius..=player_chunk.coordinates.x + radius {
						for y in player_chunk.coordinates.y - radius..=player_chunk.coordinates.y + radius {
							for z in player_chunk.coordinates.z - radius..=player_chunk.coordinates.z + radius {
								let chunk = ChunkCoordinates::new(voxject.id, vector![x, y, z], level);

								// circles look nicer
								let chunk_center = vector![x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5];
								if player_chunk != chunk
									&& player_position.metric_distance(&chunk_center) as i32 > radius
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

				if *level < 30 {
					level_chunks = level_chunks.into_iter().map(|chunk| chunk.upleveled()).collect();
				}
			}
		}

		(client_locks, tick_locks)
	}
}

impl Deref for Player {
	type Target = Connection;

	fn deref(&self) -> &Self::Target {
		&self.connection
	}
}

pub struct Connection {
	pub id: usize,
	pub name: Box<str>,

	_latency: Arc<RwLock<Duration>>,

	disconnect: RwLock<Option<OneshotSender<Option<CloseFrame<'static>>>>>,
	outgoing: Sender<ClientboundMessage>,
	incoming: Mutex<Receiver<ServerboundMessage>>,
}

#[derive(Deserialize)]
pub struct NameQuery {
	name: Box<str>,
}

impl Connection {
	#[must_use]
	pub fn _latency(&self) -> Duration {
		*self._latency.blocking_read()
	}

	pub fn is_disconnected(&self) -> bool {
		self.disconnect.blocking_read().is_none()
			|| self.outgoing.is_closed()
			|| self.incoming.blocking_lock().is_closed()
	}

	pub fn _disconnect(&self, reason: Option<CloseFrame<'static>>) {
		if let Some(disconnect) = self.disconnect.blocking_write().take() {
			let _ = disconnect.send(reason);
		}
	}

	pub fn send(&self, message: impl Into<ClientboundMessage>) {
		let _ = self.outgoing.send(message.into());
	}

	pub fn try_recv(&self) -> Result<ServerboundMessage, TryRecvError> {
		self.incoming.blocking_lock().try_recv()
	}

	pub async fn connect(
		ConnectInfo(address): ConnectInfo<SocketAddr>,
		State(sectors): State<Sectors>,
		Path(sector): Path<Box<str>>,
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

					let latency: Arc<RwLock<Duration>> = Arc::default();
					let (disconnect, handler_disconnect) = oneshot();
					let (outgoing, handler_outgoing) = channel();
					let (handler_incoming, incoming) = channel();

					static COUNTER: AtomicUsize = AtomicUsize::new(0);
					let id = COUNTER.fetch_add(1, Relaxed);

					let connection = Arc::new(Self {
						id,
						name: name.clone(),
						_latency: latency.clone(),
						disconnect: RwLock::new(Some(disconnect)),
						outgoing,
						incoming: Mutex::new(incoming),
					});

					if sector_handle.send(Event::PlayerConnected(connection)).is_err() {
						warn!("[{name}] Failed to connect to {sector:?}, sector handle has been dropped.")
					}

					tokio::spawn(Self::handle_connection(
						id,
						name.clone(),
						latency,
						handler_disconnect,
						handler_incoming,
						handler_outgoing,
						socket,
						sector_handle,
					));
				})
			},
		)
	}

	#[allow(clippy::too_many_arguments)]
	async fn handle_connection(
		id: usize,
		name: Box<str>,
		latency: Arc<RwLock<Duration>>,
		disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		incoming: Sender<ServerboundMessage>,
		outgoing: Receiver<ClientboundMessage>,
		mut socket: WebSocket,
		sector: Arc<SectorHandle>,
	) {
		info!("[{name}] Connected");

		let reason = Self::connection_loop(latency, disconnect, incoming, outgoing, &mut socket)
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
				let frame = frame.as_ref().unwrap_or(&CloseFrame {
					code: close_code::ABNORMAL,
					reason: Cow::Borrowed("Abnormal"),
				});
				info!("[{name}] Disconnected by Client: {} {}", frame.code, frame.reason);
			}
			Closed::Server(frame) => {
				{
					let frame = frame.as_ref().unwrap_or(&CloseFrame {
						code: close_code::ERROR,
						reason: Cow::Borrowed("Unknown"),
					});
					info!("[{name}] Disconnected by Server: {} {}", frame.code, frame.reason);
				}
				let _ = socket.send(Message::Close(frame)).await;
			}
		};

		let _ = sector.send(Event::PlayerDisconnected(id));
	}

	async fn connection_loop(
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
				message = outgoing.recv() => {
					let message = message.ok_or(Error::Dropped)?;
					let bytes = bincode::serialize(&message).map_err(|error| Error::Unknown(error.into()))?;
					socket.send(Message::Binary(bytes)).await?;
				}
			}
		}
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
