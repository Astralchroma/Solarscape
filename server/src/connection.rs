use log::info;
use solarscape_shared::connection::Connection;
use solarscape_shared::protocol::{DisconnectReason, Message, PROTOCOL_VERSION};
use std::{convert::Infallible, io, net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, mpsc::error::SendError, oneshot};
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpListener, net::TcpStream};

use DisconnectReason::InternalError;

pub struct ServerConnection {
	disconnect: oneshot::Sender<DisconnectReason>,
	send: mpsc::UnboundedSender<Arc<[u8]>>,
	receive: mpsc::UnboundedReceiver<Message>,
}

impl ServerConnection {
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

	pub async fn r#await(incoming: mpsc::UnboundedSender<Self>) -> Result<Infallible, io::Error> {
		// TODO: config or a cli option or something idk
		let socket = TcpListener::bind("[::]:23500").await?;
		info!("Listening on [::]:23500");

		loop {
			let (stream, address) = socket.accept().await?;
			tokio::spawn(Self::accept(incoming.clone(), stream, address));
		}
	}

	async fn accept(incoming: mpsc::UnboundedSender<Self>, mut stream: TcpStream, address: SocketAddr) {
		match Self::version_handshake(&mut stream).await {
			Err(error) => {
				info!("({address}) Failed version handshake! An error occurred: {error:?}");
				let _ = stream.shutdown().await;
				return;
			}
			Ok(version_match) => {
				if !version_match {
					info!("({address}) Failed version handshake!");
					let _ = stream.shutdown().await;
					return;
				}
			}
		}

		let (disconnect_in, disconnect_out) = oneshot::channel();
		let (send_in, send_out) = mpsc::unbounded_channel();
		let (receive_in, receive_out) = mpsc::unbounded_channel();

		tokio::spawn(Self::process(address, stream, disconnect_out, send_out, receive_in));

		let connection = ServerConnection {
			disconnect: disconnect_in,
			send: send_in,
			receive: receive_out,
		};

		match incoming.send(connection) {
			Ok(_) => {}
			Err(SendError(connection)) => connection.disconnect(InternalError),
		}
	}

	async fn version_handshake(stream: &mut TcpStream) -> Result<bool, io::Error> {
		let version = stream.read_u16().await?;

		match version == PROTOCOL_VERSION {
			false => {
				stream.write_u8(0).await?;
				stream.write_u16(PROTOCOL_VERSION).await?;
				stream.shutdown().await?;
				Ok(false)
			}
			true => {
				stream.write_u8(1).await?;
				Ok(true)
			}
		}
	}
}

impl Connection for ServerConnection {}
