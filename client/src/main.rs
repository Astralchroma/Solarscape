use anyhow::Result;
use solarscape_shared::{Clientbound, DisconnectReason, PacketRead, PacketWrite, Serverbound, PROTOCOL_VERSION};
use std::{panic, process::exit};
use tokio::{io::AsyncWriteExt, net::TcpStream};
use DisconnectReason::ProtocolViolation;

#[tokio::main]
async fn main() -> Result<()> {
	// If there is a panic we should always exit immediately, tokio won't do this for us.
	let default_panic = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		default_panic(info);
		exit(1);
	}));

	let mut socket = TcpStream::connect("[::1]:23500").await?;

	println!("Connecting to [::1]:23500");

	socket.write_packet(&Serverbound::Hello(*PROTOCOL_VERSION)).await?;

	match socket.read_packet().await? {
		Clientbound::Hello => {}
		Clientbound::Disconnected(reason) => {
			eprintln!("Disconnected: {reason:?}");
			socket.shutdown().await?;
			return Ok(());
		}
		_ => {
			socket
				.write_packet(&Serverbound::Disconnected(ProtocolViolation))
				.await?;
			socket.shutdown().await?;
			return Ok(());
		}
	}

	print!("Connected to [::1]:23500");

	loop {
		match socket.read_packet().await? {
			Clientbound::Hello => {
				socket
					.write_packet(&Serverbound::Disconnected(ProtocolViolation))
					.await?;
				socket.shutdown().await?;
				return Ok(());
			}
			Clientbound::Disconnected(reason) => {
				eprintln!("Disconnected: {reason:?}");
				socket.shutdown().await?;
				return Ok(());
			}
		}
	}
}
