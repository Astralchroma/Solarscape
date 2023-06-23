use anyhow::Result;
use solarscape_shared::{
	data::DisconnectReason::ProtocolViolation,
	io::{PacketRead, PacketWrite},
	Clientbound, Serverbound, PROTOCOL_VERSION,
};
use std::{panic, process::exit};
use tokio::{io::AsyncWriteExt, net::TcpStream};

#[tokio::main]
async fn main() -> Result<()> {
	// If there is a panic we should always exit immediately, tokio won't do this for us.
	let default_panic = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		default_panic(info);
		exit(1);
	}));

	let mut socket = TcpStream::connect("[::1]:23500").await?;
	let mut sectors = vec![];

	println!("Connecting to [::1]:23500");

	socket.write_packet(&Serverbound::Hello(*PROTOCOL_VERSION)).await?;

	loop {
		match socket.read_packet().await? {
			Clientbound::Hello => break,
			Clientbound::Disconnected(reason) => {
				eprintln!("Disconnected: {reason:?}");
				socket.shutdown().await?;
				return Ok(());
			}
			Clientbound::UpdateSectorMeta(sector_meta) => {
				println!("Received sector: {}", sector_meta.display_name);
				sectors.push(sector_meta);
			}
			_ => {
				socket
					.write_packet(&Serverbound::Disconnected(ProtocolViolation))
					.await?;
				socket.shutdown().await?;
				return Ok(());
			}
		}
	}

	print!("Connected to [::1]:23500");

	loop {
		match socket.read_packet().await? {
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
	}
}
