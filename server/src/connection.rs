use crate::world::World;
use anyhow::Result;
use log::{info, warn};
use solarscape_shared::{
	io::{PacketRead, PacketWrite},
	protocol::{
		Clientbound,
		DisconnectReason::{self, Disconnected, InternalError, ProtocolViolation, VersionMismatch},
		Serverbound, PROTOCOL_VERSION,
	},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
	io::AsyncWriteExt,
	net::TcpStream,
	select,
	sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};

pub struct Connection {
	pub address: SocketAddr,
	send: UnboundedSender<Clientbound>,
	disconnect: UnboundedSender<Result<DisconnectReason>>,
}

impl Connection {
	pub async fn accept(server: Arc<World>, stream: TcpStream, address: SocketAddr) -> Option<Arc<Connection>> {
		let (send_sender, send) = mpsc::unbounded_channel();
		let (disconnect_sender, disconnect) = mpsc::unbounded_channel();
		let (receive_sender, mut receive) = mpsc::unbounded_channel();

		let connection = Arc::new(Connection {
			address,
			send: send_sender,
			disconnect: disconnect_sender,
		});

		tokio::spawn(
			connection
				.clone()
				.oversee_communication(stream, send, disconnect, receive_sender),
		);

		match connection.handshake(server, &mut receive).await {
			Err(error) => {
				connection.disconnect(Err(error));
				return None;
			}
			Ok(reason) => {
				if let Some(reason) = reason {
					connection.disconnect(Ok(reason));
					return None;
				}
			}
		}

		tokio::spawn(connection.clone().oversee_processing(receive));

		Some(connection)
	}

	async fn oversee_communication(
		self: Arc<Self>,
		mut stream: TcpStream,
		send: UnboundedReceiver<Clientbound>,
		disconnect: UnboundedReceiver<Result<DisconnectReason>>,
		receive: UnboundedSender<Serverbound>,
	) {
		let reason = match Connection::communicate(&mut stream, send, disconnect, receive).await {
			Ok(reason) => {
				info!("[{}] Disconnected! Reason: {reason:?}", self.address);
				reason
			}
			Err(error) => {
				warn!("[{}] Disconnected! Unhandled error: {error:?}", self.address);
				InternalError
			}
		};

		let _ = stream.write_packet(&Clientbound::Disconnected { reason }).await;
		let _ = stream.shutdown().await;
	}

	async fn communicate(
		stream: &mut TcpStream,
		mut send: UnboundedReceiver<Clientbound>,
		mut disconnect: UnboundedReceiver<Result<DisconnectReason>>,
		receive: UnboundedSender<Serverbound>,
	) -> Result<DisconnectReason> {
		loop {
			select! {
				disconnect = disconnect.recv() => return disconnect.ok_or(ChannelClosed)?,
				packet = send.recv() => match packet.ok_or(ChannelClosed)? {
					Clientbound::Disconnected { reason } => return Ok(reason),
					packet => stream.write_packet(&packet).await?,
				},
				packet = stream.read_packet() => receive.send(packet?)?,
			}
		}
	}

	async fn handshake(
		self: &Arc<Self>,
		server: Arc<World>,
		receive: &mut UnboundedReceiver<Serverbound>,
	) -> Result<Option<DisconnectReason>> {
		info!("[{}] Connecting!", self.address);

		let protocol_version = match receive.recv().await.ok_or(ChannelClosed)? {
			Serverbound::Hello {
				major_version: protocol_version,
			} => protocol_version,
			_ => return Ok(Some(ProtocolViolation)),
		};

		if protocol_version != *PROTOCOL_VERSION {
			return Ok(Some(VersionMismatch(*PROTOCOL_VERSION)));
		}

		for sector in server.sectors.iter() {
			self.send(Clientbound::SyncSector {
				name: sector.name.clone(),
				display_name: sector.display_name.clone(),
			})
		}

		let sector = &server.sectors[0];

		self.send(Clientbound::ActiveSector {
			name: sector.name.clone(),
		});

		for chunk in sector.object.chunks.values() {
			self.send(Clientbound::SyncChunk {
				grid_position: chunk.grid_position,
				data: *chunk.data.read().await,
			})
		}

		info!("[{}] Connected!", self.address);

		Ok(None)
	}

	async fn oversee_processing(self: Arc<Self>, receive: UnboundedReceiver<Serverbound>) {
		self.disconnect(Connection::process(receive).await);
	}

	async fn process(mut receive: UnboundedReceiver<Serverbound>) -> Result<DisconnectReason> {
		match receive.recv().await.ok_or(ChannelClosed)? {
			Serverbound::Hello { .. } => Ok(ProtocolViolation),
			Serverbound::Disconnected { .. } => Ok(Disconnected),
		}
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
