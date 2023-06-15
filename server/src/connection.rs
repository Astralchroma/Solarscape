use anyhow::Result;
use solarscape_shared::{
	data::{
		DisconnectReason,
		DisconnectReason::{InternalError, ProtocolViolation, VersionMismatch},
	},
	io::{PacketRead, PacketWrite},
	Clientbound::{self, Hello},
	Serverbound, PROTOCOL_VERSION,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
	io::AsyncWriteExt,
	net::TcpStream,
	select,
	sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};

pub struct Connection {
	address: SocketAddr,
	send: UnboundedSender<Clientbound>,
	disconnect: UnboundedSender<Result<DisconnectReason>>,
}

impl Connection {
	pub async fn new(mut stream: TcpStream, address: SocketAddr) {
		let (send_sender, send) = mpsc::unbounded_channel();
		let (disconnect_sender, disconnect) = mpsc::unbounded_channel();
		let (receive_sender, receive) = mpsc::unbounded_channel();

		let connection = Arc::new(Connection {
			address,
			send: send_sender,
			disconnect: disconnect_sender,
		});

		{
			let connection = connection.clone();
			tokio::spawn(async move { connection.disconnect(connection.connection_manager(receive).await) });
		}

		let reason = match Connection::communication_manager(&mut stream, send, disconnect, receive_sender).await {
			Ok(reason) => {
				println!("[{}] Disconnected! Reason: {reason:?}", connection.identity());
				reason
			}
			Err(error) => {
				eprintln!("[{}] Disconnected! Unhandled error: {error:?}", connection.identity());
				InternalError
			}
		};

		let _ = stream.write_packet(&Clientbound::Disconnected(reason)).await;
		let _ = stream.shutdown().await;
	}

	async fn communication_manager(
		stream: &mut TcpStream,
		mut send: UnboundedReceiver<Clientbound>,
		mut disconnect: UnboundedReceiver<Result<DisconnectReason>>,
		receive: UnboundedSender<Serverbound>,
	) -> Result<DisconnectReason> {
		loop {
			select! {
				disconnect = disconnect.recv() => return disconnect.ok_or(ChannelClosed)?,
				packet = send.recv() => match packet.ok_or(ChannelClosed)? {
					Clientbound::Disconnected(reason) => return Ok(reason),
					packet => stream.write_packet(&packet).await?,
				},
				packet = stream.read_packet() => receive.send(packet?)?,
			}
		}
	}

	async fn connection_manager(
		self: &Arc<Self>,
		mut receive: UnboundedReceiver<Serverbound>,
	) -> Result<DisconnectReason> {
		println!("[{}] Connecting!", self.identity());

		let protocol_version = match receive.recv().await.ok_or(ChannelClosed)? {
			Serverbound::Hello(protocol_version) => protocol_version,
			_ => return Ok(ProtocolViolation),
		};

		if protocol_version != *PROTOCOL_VERSION {
			return Ok(VersionMismatch(*PROTOCOL_VERSION));
		}

		self.send(Hello);

		println!("[{}] Connected!", self.identity());

		loop {
			match self.process_packet(receive.recv().await.ok_or(ChannelClosed)?) {
				Err(_) => return Ok(ProtocolViolation), // Assume error is ProtocolViolation
				Ok(reason) => {
					if let Some(reason) = reason {
						return Ok(reason);
					}
				}
			}
		}
	}

	fn process_packet(&self, packet: Serverbound) -> Result<Option<DisconnectReason>> {
		return match packet {
			Serverbound::Disconnected(_) => Ok(Some(DisconnectReason::Disconnected)),
			_ => Ok(Some(ProtocolViolation)),
		};
	}

	pub const fn address(&self) -> SocketAddr {
		self.address
	}

	pub fn identity(&self) -> String {
		self.address.to_string()
	}

	pub fn send(&self, message: Clientbound) {
		let _ = self.send.send(message);
	}

	fn disconnect(&self, disconnect: Result<DisconnectReason>) {
		let _ = self.disconnect.send(disconnect);
	}
}

#[derive(Debug, thiserror::Error)]
#[error("channel closed unexpectedly")]
pub struct ChannelClosed;
