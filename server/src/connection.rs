//! Largely the same as the client Connection, except specific to the client, like using `axum`'s `WebSocket`s, and
//! handling events through a channel instead of the `winit` `EventLoop`. If you make a change here, check if that
//! change is needed on the client.

use crate::Worlds;
use axum::extract::ws::{close_code, CloseFrame, Message as Frame, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::{http::StatusCode, response::IntoResponse, response::Response};
use log::{error, info, warn};
use rand::distributions::{Alphanumeric, DistString};
use solarscape_shared::messages::{clientbound::ClientboundMessage, serverbound::ServerboundMessage};
use std::{borrow::Cow, sync::atomic::AtomicU64, sync::atomic::Ordering::Relaxed, sync::Arc, time::Duration};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use tokio::sync::oneshot::{channel as oneshot, Receiver as OneshotReceiver, Sender as OneshotSender};
use tokio::{pin, select, sync::mpsc::error::TryRecvError, time::interval, time::Instant};

pub struct Connection {
	close: OneshotSender<Option<CloseFrame<'static>>>,

	// Not a Arc<RwLock<Duration>> because std::sync::RwLock sucks and I don't want to use tokio::Sync::RwLock because
	// then I need separate async and sync functions, also doesn't really need to be a u64, however the
	// std::time::Duration uses u64 for millis and I don't want to spam `as u64` everywhere.
	latency: Arc<AtomicU64>,

	outgoing: Sender<ClientboundMessage>,

	incoming: Receiver<Event>,
}

impl Connection {
	pub fn close(self, reason: Option<CloseFrame<'static>>) {
		let _ = self.close.send(reason);
	}

	pub fn latency(&self) -> Duration {
		Duration::from_millis(self.latency.load(Relaxed))
	}

	pub fn send(&self, message: impl Into<ClientboundMessage>) {
		let _ = self.outgoing.send(message.into());
	}

	pub fn recv(&mut self) -> impl Iterator<Item = Event> + '_ {
		pub struct ReceiveIterator<'c>(&'c mut Connection);

		impl Iterator for ReceiveIterator<'_> {
			type Item = Event;

			fn next(&mut self) -> Option<Self::Item> {
				self.0.incoming.try_recv().map_or_else(
					|error| match error {
						TryRecvError::Empty => None,
						TryRecvError::Disconnected => Some(Event::Closed),
					},
					Some,
				)
			}
		}

		ReceiveIterator(self)
	}

	pub async fn await_connections(
		State(worlds): State<Worlds>,
		Path(world): Path<String>,
		socket: WebSocketUpgrade,
	) -> Response {
		let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
		worlds.get(&world).map_or_else(
			|| StatusCode::NOT_FOUND.into_response(),
			|world_handle| {
				let world_handle = world_handle.clone();
				socket.on_upgrade(|socket| async move {
					let latency = Arc::new(AtomicU64::default());
					let (close_send, close_recv) = oneshot();
					let (outgoing_send, outgoing_recv) = channel();
					let (incoming_send, incoming_recv) = channel();

					let connection = Self {
						close: close_send,
						latency: latency.clone(),
						outgoing: outgoing_send,
						incoming: incoming_recv,
					};

					if world_handle.send(connection).await.is_err() {
						return;
					}

					Self::connection_handler(name, socket, close_recv, latency, outgoing_recv, incoming_send).await;
				})
			},
		)
	}

	async fn connection_handler(
		name: String,
		mut socket: WebSocket,
		disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		latency: Arc<AtomicU64>,
		outgoing: Receiver<ClientboundMessage>,
		incoming: Sender<Event>,
	) {
		info!("[{name}] Connected");

		let reason = Self::connection(&mut socket, disconnect, latency, outgoing, incoming)
			.await
			.unwrap_or_else(|error| match error {
				Error::Dropped => {
					warn!("[{name}] Connection was dropped unexpectedly!");
					Closed::Server(Some(CloseFrame {
						code: close_code::ERROR,
						reason: Cow::Borrowed("Internal Error"),
					}))
				}
				Error::InvalidData => Closed::Server(Some(CloseFrame {
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
				let _ = socket.send(Frame::Close(frame)).await;
			}
		};
	}

	async fn connection(
		socket: &mut WebSocket,
		mut disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		latency: Arc<AtomicU64>,
		mut outgoing: Receiver<ClientboundMessage>,
		incoming: Sender<Event>,
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
							socket.send(Frame::Ping(Vec::from(&ping))).await?;
						}
						Some(_) => return Err(Error::TimedOut),
					}
				}
				message = outgoing.recv() => {
					let message = message.ok_or(Error::Dropped)?;
					socket.send(Frame::Binary(
						bincode::serialize(&message)
							.map_err(|error| Error::Unknown(error.into()))?
					)).await?;
				}
				message = socket.recv() => {
					let message = match message {
						None => return Ok(Closed::Client(None)),
						Some(message) => message,
					};

					match message? {
						Frame::Text(_) => return Err(Error::InvalidData),
						Frame::Binary(data) => {
							incoming.send(Event::Message(
								bincode::deserialize(&data).map_err(|_| Error::InvalidData)?
							)).map_err(|_| Error::Dropped)?;
						},
						Frame::Ping(data) => socket.send(Frame::Pong(data)).await?,
						Frame::Pong(pong) => {
							match pending_pong {
								None => return Err(Error::InvalidData),
								Some((expected_pong, time)) => {
									if pong != expected_pong {
										return Err(Error::InvalidData);
									}

									let round_trip_time = Instant::now() - time;
									last_pings.copy_within(1.., 0);
									last_pings[11] = round_trip_time;

									latency.store(
										(last_pings.iter().fold(
											Duration::ZERO,
											|total, ping| total + *ping) / 12
										).as_millis() as u64,
										Relaxed
									);

									pending_pong = None;
								}
							}
						}
						Frame::Close(reason) => return Ok(Closed::Client(reason)),
					}
				}
			}
		}
	}
}

enum Closed {
	Client(Option<CloseFrame<'static>>),
	Server(Option<CloseFrame<'static>>),
}

pub enum Error {
	Dropped,
	InvalidData,
	TimedOut,
	Unknown(anyhow::Error),
}

impl From<axum::Error> for Error {
	fn from(error: axum::Error) -> Self {
		Self::Unknown(error.into())
	}
}

pub enum Event {
	/// The connection has already been closed, this will be repeated
	Closed,
	Message(ServerboundMessage),
}
