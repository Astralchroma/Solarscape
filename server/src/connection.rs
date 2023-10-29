use bincode::{config::standard, error::DecodeError, error::EncodeError};
use log::info;
use solarscape_shared::protocol::{Clientbound, DisconnectReason, Serverbound, PACKET_LENGTH_LIMIT, PROTOCOL_VERSION};
use std::{convert::Infallible, io, net::SocketAddr};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufStream};
use tokio::{net::TcpListener, net::TcpStream, select, sync::mpsc, sync::mpsc::error::SendError, sync::oneshot};
use ConnectionError::{Decode, Encode, Io, Send};
use DisconnectReason::{ConnectionLost, InternalError, ProtocolViolation, VersionMismatch};

pub struct Connection {
	address: SocketAddr,
	disconnect: oneshot::Sender<DisconnectReason>,
	send: mpsc::UnboundedSender<Clientbound>,
	receive: mpsc::UnboundedReceiver<Serverbound>,
}

impl Connection {
	pub fn address(&self) -> &SocketAddr {
		&self.address
	}

	pub fn disconnect(self, reason: DisconnectReason) {
		let _ = self.disconnect.send(reason);
	}

	pub fn is_alive(&self) -> bool {
		!self.disconnect.is_closed()
	}

	pub fn send(&self, packet: Clientbound) {
		let _ = self.send.send(packet);
	}

	pub fn receive(&mut self) -> &mut mpsc::UnboundedReceiver<Serverbound> {
		&mut self.receive
	}

	pub async fn r#await(incoming: mpsc::UnboundedSender<Self>) -> Result<Infallible, io::Error> {
		// TODO: config or a cli option or something idk
		let socket = TcpListener::bind("[::]:23500").await?;
		info!("Listening on [::]:23500");

		loop {
			let (stream, address) = socket.accept().await?;
			tokio::spawn(Self::accept(incoming.clone(), stream, address));
		}
	}

	async fn accept(incoming: mpsc::UnboundedSender<Self>, stream: TcpStream, address: SocketAddr) {
		let (disconnect_in, disconnect_out) = oneshot::channel();
		let (send_in, send_out) = mpsc::unbounded_channel();
		let (receive_in, receive_out) = mpsc::unbounded_channel();

		tokio::spawn(Self::process(address, stream, disconnect_out, send_out, receive_in));

		let mut connection = Connection {
			address,
			disconnect: disconnect_in,
			send: send_in,
			receive: receive_out,
		};

		if let Err(reason) = connection.handshake().await {
			connection.disconnect(reason);
			return;
		}

		match incoming.send(connection) {
			Ok(_) => {}
			Err(SendError(connection)) => connection.disconnect(InternalError),
		}
	}

	async fn process(
		address: SocketAddr,
		stream: TcpStream,
		disconnect: oneshot::Receiver<DisconnectReason>,
		send: mpsc::UnboundedReceiver<Clientbound>,
		receive: mpsc::UnboundedSender<Serverbound>,
	) {
		let mut stream = BufStream::new(stream);

		let reason = match Self::process_inner(&mut stream, disconnect, send, receive).await {
			Ok(reason) => reason,
			Err(error) => match error {
				Decode(_) => ProtocolViolation,
				Encode(_) | Send(_) => InternalError,
				Io(_) => ConnectionLost,
			},
		};

		info!("({address}) Disconnected! Reason: {reason:?}");

		if let Ok(ref buffer) = bincode::encode_to_vec(Clientbound::Disconnected { reason }, standard()) {
			let _ = stream.write_u16(buffer.len() as u16).await;
			let _ = stream.write_all(buffer).await;
		}

		let _ = stream.shutdown().await;
	}

	/// process_inner() actually does the communicating and passes any errors up to the process() function which handles
	/// errors and disconnects. The alternative is a lot match and if let statements which is really messy.
	/// I couldn't think of a better name for this. - Ferra  
	async fn process_inner(
		stream: &mut BufStream<TcpStream>,
		mut disconnect: oneshot::Receiver<DisconnectReason>,
		mut send: mpsc::UnboundedReceiver<Clientbound>,
		receive: mpsc::UnboundedSender<Serverbound>,
	) -> Result<DisconnectReason, ConnectionError> {
		loop {
			select! {
				biased;
				reason = &mut disconnect => return Ok(reason.unwrap_or(InternalError)),
				packet = send.recv() => {
					let packet = match packet {
						Some(ref packet) => packet,
						None => return Ok(InternalError),
					};
					let buffer = bincode::encode_to_vec(packet, standard())?;
					stream.write_u16(buffer.len() as u16).await?;
					stream.write_all(&buffer).await?;
					stream.flush().await?;
				},
				length = stream.read_u16() => {
					let length = match length {
						Ok(length) => length as usize,
						Err(_) => return Ok(ConnectionLost),
					};
					if length > PACKET_LENGTH_LIMIT {
						return Ok(ProtocolViolation)
					}
					let mut buffer = vec![0; length];
					stream.read_exact(&mut buffer).await?;
					let packet = bincode::decode_from_slice(&buffer, standard())?.0;
					receive.send(packet)?;
				},
			}
		}
	}

	async fn handshake(&mut self) -> Result<(), DisconnectReason> {
		let protocol_version = match self.receive.recv().await.ok_or(ConnectionLost)? {
			Serverbound::Hello { major_version } => major_version,
			_ => return Err(ProtocolViolation),
		};

		if protocol_version != PROTOCOL_VERSION {
			return Err(VersionMismatch(PROTOCOL_VERSION));
		}

		Ok(())
	}
}

#[derive(Debug, Error)]
#[error(transparent)]
enum ConnectionError {
	Encode(#[from] EncodeError),
	Io(#[from] io::Error),
	Decode(#[from] DecodeError),
	Send(#[from] SendError<Serverbound>),
}
