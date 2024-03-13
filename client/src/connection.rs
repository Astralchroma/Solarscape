//! Largely the same as the server Connection, except specific to the client, like using `tokio-tungstenite`'s
//! `WebSocket`s, and handling events through the `winit` `EventLoop`. If you make a change here, check if that change
//! is needed on the server.

use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use solarscape_shared::messages::{clientbound::ClientboundMessage, serverbound::ServerboundMessage};
use std::{borrow::Cow, sync::atomic::AtomicU64, sync::atomic::Ordering::Relaxed, sync::Arc, time::Duration};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use tokio::sync::oneshot::{channel as oneshot, Receiver as OneshotReceiver, Sender as OneshotSender};
use tokio::{net::TcpStream, pin, select, time::interval, time::Instant};
use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
use tungstenite::{client::IntoClientRequest, Message as Frame};
use winit::event_loop::EventLoopProxy;

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Connection {
	close: OneshotSender<Option<CloseFrame<'static>>>,

	// Not a Arc<RwLock<Duration>> because std::sync::RwLock sucks and I don't want to use tokio::Sync::RwLock because
	// then I need separate async and sync functions, also doesn't really need to be a u64, however the
	// std::time::Duration uses u64 for millis and I don't want to spam `as u64` everywhere.
	latency: Arc<AtomicU64>,

	outgoing: Sender<ServerboundMessage>,
}

impl Connection {
	pub fn close(self, reason: Option<CloseFrame<'static>>) {
		let _ = self.close.send(reason);
	}

	pub fn latency(&self) -> Duration {
		Duration::from_millis(self.latency.load(Relaxed))
	}

	pub fn send(&self, message: impl Into<ServerboundMessage>) {
		let _ = self.outgoing.send(message.into());
	}

	pub async fn new(
		request: impl IntoClientRequest + Send,
		incoming: EventLoopProxy<Event>,
	) -> Result<Self, tungstenite::Error> {
		let request = request.into_client_request()?;
		let name = request.uri().to_string();
		let (socket, _) = connect_async(request).await?;

		let latency = Arc::new(AtomicU64::default());
		let (close_send, close_recv) = oneshot();
		let (outgoing_send, outgoing_recv) = channel();

		let connection = Self {
			close: close_send,
			latency: latency.clone(),
			outgoing: outgoing_send,
		};

		tokio::spawn(Self::connection_handler(
			name,
			socket,
			close_recv,
			latency,
			outgoing_recv,
			incoming,
		));

		Ok(connection)
	}

	async fn connection_handler(
		name: String,
		mut socket: WebSocket,
		disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		latency: Arc<AtomicU64>,
		outgoing: Receiver<ServerboundMessage>,
		incoming: EventLoopProxy<Event>,
	) {
		info!("Connected to {name:?}");

		let reason = Self::connection(&mut socket, disconnect, latency, outgoing, incoming)
			.await
			.unwrap_or_else(|error| match error {
				Error::Dropped => {
					warn!("[{name}] Connection was dropped unexpectedly!");
					Closed::Client(Some(CloseFrame {
						code: CloseCode::Error,
						reason: Cow::Borrowed("Internal Error"),
					}))
				}
				Error::InvalidData => Closed::Client(Some(CloseFrame {
					code: CloseCode::Invalid,
					reason: Cow::Borrowed("Invalid Data"),
				})),
				Error::TimedOut => Closed::Client(Some(CloseFrame {
					code: CloseCode::Protocol,
					reason: Cow::Borrowed("Timed Out"),
				})),
				Error::Unknown(error) => {
					error!("[{name}] Error in connection handler: {error}");
					Closed::Client(Some(CloseFrame {
						code: CloseCode::Error,
						reason: Cow::Borrowed("Internal Error"),
					}))
				}
			});

		match reason {
			Closed::Client(frame) => {
				{
					let frame = frame.as_ref().unwrap_or(&CloseFrame {
						code: CloseCode::Error,
						reason: Cow::Borrowed("Unknown"),
					});
					info!("Disconnected by Client: {} {}", frame.code, frame.reason);
				}
				let _ = socket.send(Frame::Close(frame)).await;
			}
			Closed::Server(frame) => {
				let frame = frame.as_ref().unwrap_or(&CloseFrame {
					code: CloseCode::Abnormal,
					reason: Cow::Borrowed("Abnormal"),
				});
				info!("Disconnected by Server: {} {}", frame.code, frame.reason);
			}
		};
	}

	async fn connection(
		socket: &mut WebSocket,
		mut disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		latency: Arc<AtomicU64>,
		mut outgoing: Receiver<ServerboundMessage>,
		incoming: EventLoopProxy<Event>,
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
					return Ok(Closed::Client(reason));
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
				message = socket.next() => {
					let message = match message {
						None => return Ok(Closed::Server(None)),
						Some(message) => message,
					};

					match message? {
						Frame::Text(_) => return Err(Error::InvalidData),
						Frame::Binary(data) => {
							incoming.send_event(Event::Message(
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
						Frame::Close(reason) => return Ok(Closed::Server(reason)),
						Frame::Frame(_) => unreachable!(),
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

impl From<tungstenite::Error> for Error {
	fn from(error: tungstenite::Error) -> Self {
		Self::Unknown(error.into())
	}
}

pub enum Event {
	Message(ClientboundMessage),
}
