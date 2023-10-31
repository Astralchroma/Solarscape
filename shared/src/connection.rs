use crate::protocol::{DisconnectReason, Message, Protocol, PACKET_LENGTH_LIMIT};
use async_trait::async_trait;
use bincode::{config::standard, error::DecodeError};
use log::info;
use std::{io, net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::sync::{mpsc, mpsc::error::SendError, oneshot};
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, io::BufStream, net::TcpStream, select};

use ConnectionError::{Decode, Dropped, Io, OversizedPacket, Reason, Send};
use DisconnectReason::{ConnectionLost, InternalError, ProtocolViolation};

#[async_trait]
pub trait Connection {
	async fn process(
		address: SocketAddr,
		stream: TcpStream,
		disconnect: oneshot::Receiver<DisconnectReason>,
		send: mpsc::UnboundedReceiver<Arc<[u8]>>,
		receive: mpsc::UnboundedSender<Message>,
	) {
		let mut stream = BufStream::new(stream);

		info!("({address}) Connected!");

		match Self::communicate(&mut stream, disconnect, send, receive).await {
			Ok(reason) => {
				info!("({address}) Disconnected by peer! Reason: {reason:?}");
			}
			Err(error) => {
				let reason = match error {
					Decode(_) | OversizedPacket => ProtocolViolation,
					Dropped | Send(_) => InternalError,
					Io(_) => ConnectionLost,
					Reason(reason) => reason,
				};

				if let Ok(buffer) = bincode::encode_to_vec(Protocol::Disconnected(reason), standard()) {
					let _ = stream.write_u16(buffer.len() as u16).await;
					let _ = stream.write_all(&buffer).await;
				}

				info!("({address}) Disconnected due to error! {error:?}")
			}
		};

		let _ = stream.shutdown().await;
	}

	async fn communicate(
		stream: &mut BufStream<TcpStream>,
		mut disconnect: oneshot::Receiver<DisconnectReason>,
		mut send: mpsc::UnboundedReceiver<Arc<[u8]>>,
		receive: mpsc::UnboundedSender<Message>,
	) -> Result<DisconnectReason, ConnectionError> {
		loop {
			select! {
				biased;
				reason = &mut disconnect => return Err(reason.map_or(Dropped, Reason)),
				packet = send.recv() => {
					let packet = match packet {
						// If you are seeing an error just below here, it's not real, IntelliJ is just being unintellij
						Some(packet) => packet,
						None => return Err(Dropped),
					};
					stream.write_u16(packet.len() as u16).await?;
					stream.write_all(&packet).await?;
					stream.flush().await?;
				},
				length = stream.read_u16() => {
					let length = length? as usize;
					if length > PACKET_LENGTH_LIMIT {
						return Err(OversizedPacket)
					}
					let mut buffer = vec![0; length];
					stream.read_exact(&mut buffer).await?;
					match bincode::decode_from_slice(&buffer, standard())?.0 {
						Protocol::Disconnected(reason) => return Ok(reason),
						Protocol::Message(message) => receive.send(message)?,
					}
				},
			}
		}
	}
}

#[derive(Debug, Error)]
#[error(transparent)]
#[allow(clippy::large_enum_variant)] // Don't care
pub enum ConnectionError {
	Decode(#[from] DecodeError),
	#[error("connection channel was dropped")]
	Dropped,
	Io(#[from] io::Error),
	#[error("received packet was oversized")]
	OversizedPacket,
	Send(#[from] SendError<Message>),
	#[error("Disconnected with reason: {0:?}")]
	Reason(DisconnectReason),
}
