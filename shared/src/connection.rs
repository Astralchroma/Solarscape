use crate::message::{clientbound::Clientbound, serverbound::Serverbound};
use chacha20poly1305::{AeadInPlace, ChaCha20Poly1305};
use log::warn;
use serde::{de::DeserializeOwned, Serialize};
use std::{io, marker::PhantomData, ops::Deref, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt, BufStream},
	net::TcpStream,
	pin, select,
	sync::mpsc::{
		error::TryRecvError, unbounded_channel as channel, UnboundedReceiver as Receiver,
		UnboundedSender as Sender,
	},
	time::sleep,
};

pub trait ConnectionSide: Default + Send + 'static {
	type I: DeserializeOwned + Send;
	type O: Serialize + Send;

	fn next(counter: &mut NonceCounter<Self>) -> [u8; 12];
	fn peer_next(counter: &mut NonceCounter<Self>) -> [u8; 12];
}

// From what I've seen, a sequential nonce like this is *probably* fine?
//
// Apparently this can come with 2 concerns
// 1. If sent as part of the message format, the nonce can be predicted.
// 2. It tells any attacker the number of messages sent, and allows them to determine how often.
//
// I don't see either concern being relevant here as:
// 1. We don't include the nonce in the message as the client and server can determine them.
// 2. The nonce isn't secret, it can even be sent in "plaintext", if it is sent, the attacker can just read it.
// 3. The number and frequency of messages is fairly useless information.
//
// If you are some sort of encryption expert who happens to know otherwise, please do tell.
//
// The requirements of a nonce are only that it is only used once, a counter achieves that.
//
// The server's counter gets inverted, mean it counts down from max, while the client counts up from 0, this means a
// duplicate nonce should only be possible if we somehow send more then 2^96 packets.
pub struct NonceCounter<E: ConnectionSide> {
	server: u128,
	client: u128,
	_e: PhantomData<E>,
}

impl<E: ConnectionSide> NonceCounter<E> {
	fn client_next(&mut self) -> [u8; 12] {
		let nonce = u128::to_le_bytes(self.client);
		self.client += 1;
		*nonce.first_chunk()
			.expect("getting the first 12 bytes of nonce should always work as nonce should always be 16 bytes because u128 is 16 bytes")
	}

	fn server_next(&mut self) -> [u8; 12] {
		let nonce = u128::to_le_bytes(!self.server);
		self.server += 1;
		*nonce.first_chunk()
			.expect("getting the first 12 bytes of nonce should always work as nonce should always be 16 bytes because u128 is 16 bytes")
	}
}

// We initialize as 1 because a single message is sent before the connection is constructed
impl<E: ConnectionSide> Default for NonceCounter<E> {
	fn default() -> Self {
		Self {
			server: 1,
			client: 1,
			_e: PhantomData::default(),
		}
	}
}

#[derive(Default)]
pub struct ClientEnd;

impl ConnectionSide for ClientEnd {
	type I = Clientbound;
	type O = Serverbound;

	fn next(counter: &mut NonceCounter<Self>) -> [u8; 12] {
		counter.client_next()
	}

	fn peer_next(counter: &mut NonceCounter<Self>) -> [u8; 12] {
		counter.server_next()
	}
}

#[derive(Default)]
pub struct ServerEnd;

impl ConnectionSide for ServerEnd {
	type I = Serverbound;
	type O = Clientbound;

	fn next(counter: &mut NonceCounter<Self>) -> [u8; 12] {
		counter.server_next()
	}

	fn peer_next(counter: &mut NonceCounter<Self>) -> [u8; 12] {
		counter.client_next()
	}
}

pub struct Connection<E: ConnectionSide> {
	sender: Arc<ConnectionSend<E>>,
	incoming: Receiver<E::I>,
}

pub struct ConnectionSend<E: ConnectionSide> {
	outgoing: Sender<E::O>,
}

impl<E: ConnectionSide> Connection<E> {
	pub fn new(stream: TcpStream, cipher: ChaCha20Poly1305) -> Self {
		let stream = BufStream::new(stream);

		let (send_incoming, recv_incoming) = channel();
		let (send_outgoing, recv_outgoing) = channel();

		tokio::spawn(Self::handle_connection(
			stream,
			cipher,
			send_incoming,
			recv_outgoing,
		));

		Self {
			sender: Arc::new(ConnectionSend {
				outgoing: send_outgoing,
			}),
			incoming: recv_incoming,
		}
	}

	pub fn sender(&self) -> Arc<ConnectionSend<E>> {
		self.sender.clone()
	}

	pub async fn recv(&mut self) -> Option<E::I> {
		self.incoming.recv().await
	}

	pub fn try_recv(&mut self) -> Result<E::I, TryRecvError> {
		self.incoming.try_recv()
	}

	async fn handle_connection(
		mut stream: BufStream<TcpStream>,
		cipher: ChaCha20Poly1305,
		incoming: Sender<E::I>,
		outgoing: Receiver<E::O>,
	) {
		match Self::connection_loop(&mut stream, cipher, incoming, outgoing).await {
			Ok(_) => {}
			Err(error) => warn!("Error occurred in connection: {error}"),
		}

		// We're shutting down the stream either way, don't care
		let _ = stream.shutdown().await;
	}

	async fn connection_loop(
		stream: &mut BufStream<TcpStream>,
		cipher: ChaCha20Poly1305,
		incoming: Sender<E::I>,
		mut outgoing: Receiver<E::O>,
	) -> Result<Closed, ConnectionError> {
		let mut nonce_counter = NonceCounter::<E>::default();

		// read_u16_le is not cancellation safe, while we could pin the future to get around this, that would prevent
		// us from writing to the stream, so instead we read the first byte, and then the second byte later, as reading
		// a byte is cancellation safe.
		let mut length_first_byte = None;

		// The `sleep` is not cancellation safe, we can work around this by pinning them, this means they never get
		// cancelled.
		pin! {
			let keep_alive = sleep(Duration::from_secs(10));
			let time_out = sleep(Duration::from_secs(20));
		};

		loop {
			select! {
				biased;

				_ = &mut time_out => return Err(ConnectionError::TimedOut),

				_ = &mut keep_alive => {
					// A message of length 0 is treated as a keep-alive
					stream.write_u16_le(0).await?;
					stream.flush().await?;

					keep_alive.set(sleep(Duration::from_secs(10)));
				},

				message = outgoing.recv() => match message {
					Some(message) => {
						let mut buffer = bincode::serialize(&message)?;

						let nonce = E::next(&mut nonce_counter);
						cipher.encrypt_in_place((&nonce).into(), b"", &mut buffer)?;

						stream.write_u16_le(buffer.len() as u16).await?;
						stream.write_all(&buffer).await?;
						stream.flush().await?;

						keep_alive.set(sleep(Duration::from_secs(10)));
					},

					None => return Ok(Closed),
				},

				byte = stream.read_u8() => {
					let byte = byte?;

					match length_first_byte {
						// This is the first byte, set it and loop around
						None => length_first_byte = Some(byte),

						// Second byte, we have our length now
						Some(first_byte) => {
							let length = u16::from_le_bytes([first_byte, byte]);
							length_first_byte = None;

							// Length 0 = Keep Alive, don't do anything, just skip to resetting the time_out
							if length > 0 {
								let mut buffer = vec![0; length as usize];
								stream.read_exact(&mut buffer).await?;

								let nonce = E::peer_next(&mut nonce_counter);
								cipher.decrypt_in_place((&nonce).into(), b"", &mut buffer)?;

								let message = bincode::deserialize(&buffer)?;

								if incoming.send(message).is_err() {
									return Ok(Closed);
								}
							}

							time_out.set(sleep(Duration::from_secs(20)));
						}
					}
				},
			}
		}
	}
}

impl<E: ConnectionSide> ConnectionSend<E> {
	pub fn is_connected(&self) -> bool {
		!self.outgoing.is_closed()
	}

	pub fn send(&self, message: impl Into<E::O>) {
		let _ = self.outgoing.send(message.into());
	}
}

impl<E: ConnectionSide> Deref for Connection<E> {
	type Target = ConnectionSend<E>;

	fn deref(&self) -> &Self::Target {
		&self.sender
	}
}

impl<E: ConnectionSide> PartialEq for ConnectionSend<E> {
	fn eq(&self, other: &Self) -> bool {
		self.outgoing.same_channel(&other.outgoing)
	}
}

impl<E: ConnectionSide> Eq for ConnectionSend<E> {}

struct Closed;

#[derive(Debug, Error)]
#[error(transparent)]
enum ConnectionError {
	#[error("timed out")]
	TimedOut,

	Io(#[from] io::Error),

	Bincode(#[from] bincode::Error),

	#[error("encryption error")]
	Encryption,
}

impl From<chacha20poly1305::Error> for ConnectionError {
	fn from(_: chacha20poly1305::Error) -> Self {
		Self::Encryption
	}
}
