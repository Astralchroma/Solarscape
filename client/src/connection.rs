use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use solarscape_shared::messages::{clientbound::ClientboundMessage, serverbound::ServerboundMessage};
use std::{borrow::Cow, sync::Arc, time::Duration};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedReceiver as Receiver, UnboundedSender as Sender};
use tokio::sync::oneshot::{channel as oneshot, Receiver as OneshotReceiver, Sender as OneshotSender};
use tokio::{net::TcpStream, pin, select, sync::RwLock, time::interval, time::Instant};
use tokio_tungstenite::{connect_async, tungstenite, MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
use tungstenite::{client::IntoClientRequest, Message as Frame};
use winit::event_loop::EventLoopProxy;

type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Connection {
	latency: Arc<RwLock<Duration>>,
	disconnect: OneshotSender<Option<CloseFrame<'static>>>,
	outgoing: Sender<ServerboundMessage>,
}

impl Connection {
	#[must_use]
	pub fn latency(&self) -> Duration {
		*self.latency.blocking_read()
	}

	pub fn disconnect(self, reason: Option<CloseFrame<'static>>) {
		let _ = self.disconnect.send(reason);
	}

	pub fn send(&self, message: impl Into<ServerboundMessage>) {
		let _ = self.outgoing.send(message.into());
	}

	pub async fn new(
		request: impl IntoClientRequest + Send,
		incoming: EventLoopProxy<Event>,
	) -> Result<Self, tungstenite::Error> {
		let request = request.into_client_request()?;
		let name = Arc::from(request.uri().to_string());
		let (socket, _) = connect_async(request).await?;

		let latency: Arc<RwLock<_>> = Default::default();
		let (disconnect, handler_disconnect) = oneshot();
		let (outgoing, handler_outgoing) = channel();

		let connection = Self { disconnect, latency: latency.clone(), outgoing };

		tokio::spawn(Self::manage_connection_handler(
			name,
			latency,
			handler_disconnect,
			incoming,
			handler_outgoing,
			socket,
		));

		Ok(connection)
	}

	async fn manage_connection_handler(
		name: Arc<str>,
		latency: Arc<RwLock<Duration>>,
		disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		incoming: EventLoopProxy<Event>,
		outgoing: Receiver<ServerboundMessage>,
		mut socket: WebSocket,
	) {
		info!("Connected to {name:?}");

		let reason = Self::connection_handler(latency, disconnect, incoming, outgoing, &mut socket)
			.await
			.unwrap_or_else(|error| match error {
				Error::Dropped => {
					warn!("[{name}] Connection was dropped unexpectedly!");
					Closed::Client(Some(CloseFrame {
						code: CloseCode::Error,
						reason: Cow::Borrowed("Internal Error"),
					}))
				}
				Error::Invalid => Closed::Client(Some(CloseFrame {
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
					let frame = frame
						.as_ref()
						.unwrap_or(&CloseFrame { code: CloseCode::Error, reason: Cow::Borrowed("Unknown") });
					info!("Disconnected by Client: {} {}", frame.code, frame.reason);
				}
				let _ = socket.send(Frame::Close(frame)).await;
			}
			Closed::Server(frame) => {
				let frame = frame
					.as_ref()
					.unwrap_or(&CloseFrame { code: CloseCode::Abnormal, reason: Cow::Borrowed("Abnormal") });
				info!("Disconnected by Server: {} {}", frame.code, frame.reason);
			}
		};
	}

	async fn connection_handler(
		latency: Arc<RwLock<Duration>>,
		mut disconnect: OneshotReceiver<Option<CloseFrame<'static>>>,
		incoming: EventLoopProxy<Event>,
		mut outgoing: Receiver<ServerboundMessage>,
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
					let bytes = bincode::serialize(&message).map_err(|error| Error::Unknown(error.into()))?;
					socket.send(Frame::Binary(bytes)).await?;
				}
				message = socket.next() => {
					let message = match message {
						None => return Ok(Closed::Server(None)),
						Some(message) => message,
					};

					match message? {
						Frame::Text(_) => return Err(Error::Invalid),
						Frame::Binary(data) => {
							let message = bincode::deserialize(&data).map_err(|_| Error::Invalid)?;
							incoming.send_event(Event::Message(message)).map_err(|_| Error::Dropped)?;
						},
						Frame::Ping(data) => socket.send(Frame::Pong(data)).await?,
						Frame::Pong(pong) => {
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
	Invalid,
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
