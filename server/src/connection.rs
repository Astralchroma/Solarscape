use anyhow::Result;
use log::info;
use solarscape_shared::io::{PacketRead, PacketWrite};
use solarscape_shared::protocol::DisconnectReason::{ConnectionLost, ProtocolViolation, VersionMismatch};
use solarscape_shared::protocol::{Clientbound, DisconnectReason, Serverbound, PROTOCOL_VERSION};
use std::{convert::Infallible, io, net::SocketAddr};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::{net::TcpListener, net::TcpStream, select};

pub struct Connection {
	address: SocketAddr,
	send_in: UnboundedSender<Clientbound>,
	receive_out: UnboundedReceiver<Serverbound>,
}

impl Connection {
	pub fn address(&self) -> &SocketAddr {
		&self.address
	}

	pub fn send(&self, packet: Clientbound) {
		// TODO: This is dumb
		if let Clientbound::Disconnected { .. } = packet {
			return;
		}

		if self.send_in.send(packet).is_err() {
			todo!("handle errors lol")
		}
	}

	pub fn receive(&mut self) -> &mut UnboundedReceiver<Serverbound> {
		&mut self.receive_out
	}

	pub fn disconnect(self, reason: DisconnectReason) {
		let _ = self.send_in.send(Clientbound::Disconnected { reason });
	}

	async fn handshake(&mut self) -> Result<(), DisconnectReason> {
		let protocol_version = match self.receive_out.recv().await.ok_or(ConnectionLost)? {
			Serverbound::Hello { major_version } => major_version,
			_ => return Err(ProtocolViolation),
		};

		if protocol_version != *PROTOCOL_VERSION {
			return Err(VersionMismatch(*PROTOCOL_VERSION));
		}

		Ok(())
	}

	pub async fn r#await(incoming: UnboundedSender<Self>) -> Result<Infallible, io::Error> {
		// TODO: config or a cli option or something idk
		let socket = TcpListener::bind("[::]:23500").await?;
		info!("Listening on [::]:23500");

		loop {
			let (stream, address) = socket.accept().await?;
			tokio::spawn(Self::accept(incoming.clone(), stream, address));
		}
	}

	async fn accept(incoming: UnboundedSender<Self>, stream: TcpStream, address: SocketAddr) {
		let (send_in, send_out) = mpsc::unbounded_channel();
		let (receive_in, receive_out) = mpsc::unbounded_channel();

		tokio::spawn(Self::process(stream, send_out, receive_in));

		let mut connection = Connection {
			address,
			send_in,
			receive_out,
		};

		if let Err(reason) = connection.handshake().await {
			connection.disconnect(reason);
			return;
		}

		let _ = incoming.send(connection);
	}

	async fn process(
		mut stream: TcpStream,
		mut send_out: UnboundedReceiver<Clientbound>,
		receive_in: UnboundedSender<Serverbound>,
	) -> Result<Infallible> {
		loop {
			select! {
				packet = send_out.recv() => stream.write_packet(&packet.ok_or(ChannelClosed)?).await?,
				packet = stream.read_packet() => receive_in.send(packet?)?,
			}
		}
	}
}

#[derive(Debug, thiserror::Error)]
#[error("channel closed unexpectedly")]
pub struct ChannelClosed;
