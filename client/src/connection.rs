use solarscape_shared::protocol::PROTOCOL_VERSION;
use solarscape_shared::{connection::Connection, protocol::DisconnectReason, protocol::Message};
use std::{io, sync::Arc};
use thiserror::Error;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpStream, net::ToSocketAddrs, sync::mpsc, sync::oneshot};

pub struct ClientConnection {
	disconnect: oneshot::Sender<DisconnectReason>,
	send: mpsc::UnboundedSender<Arc<[u8]>>,
	receive: mpsc::UnboundedReceiver<Message>,
}

impl ClientConnection {
	pub fn disconnect(self, reason: DisconnectReason) {
		let _ = self.disconnect.send(reason);
	}

	pub fn is_alive(&self) -> bool {
		!self.disconnect.is_closed()
	}

	pub fn send(&self, packet: Arc<[u8]>) {
		let _ = self.send.send(packet);
	}

	pub fn receive(&mut self) -> &mut mpsc::UnboundedReceiver<Message> {
		&mut self.receive
	}

	pub async fn connect<A: ToSocketAddrs>(address: A) -> Result<Self, ConnectionError> {
		let mut stream = TcpStream::connect(address).await?;
		stream.write_u16(PROTOCOL_VERSION).await?;

		if stream.read_u8().await? == 0 {
			let server_version = stream.read_u16().await?;
			return Err(ConnectionError::VersionMismatch(PROTOCOL_VERSION, server_version));
		}

		let (disconnect_in, disconnect_out) = oneshot::channel();
		let (send_in, send_out) = mpsc::unbounded_channel();
		let (receive_in, receive_out) = mpsc::unbounded_channel();

		tokio::spawn(Self::process(
			stream.peer_addr()?,
			stream,
			disconnect_out,
			send_out,
			receive_in,
		));

		Ok(ClientConnection {
			disconnect: disconnect_in,
			send: send_in,
			receive: receive_out,
		})
	}
}

impl Connection for ClientConnection {}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ConnectionError {
	Io(#[from] io::Error),
	#[error("version mismatch, client is {0}, server was {1}")]
	VersionMismatch(u16, u16),
}
