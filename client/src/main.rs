use anyhow::Result;
use solarscape_shared::protocol::PROTOCOL_VERSION;
use solarscape_shared::{
	io::{PacketRead, PacketWrite},
	protocol::{Clientbound, DisconnectReason::ProtocolViolation, Serverbound},
};
use std::{collections::HashMap, panic, process::exit};
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
	let mut sector_id;
	let mut chunks = HashMap::new();

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
			Clientbound::SyncSector(sector_meta) => {
				println!("Received sector \"{}\"", sector_meta.display_name);
				sectors.push(sector_meta);
			}
			Clientbound::ActiveSector(active_sector) => {
				println!("Switched to sector \"{}\"", sectors[active_sector].display_name);
				sector_id = active_sector;
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
			Clientbound::SyncChunk(chunk) => {
				println!("Received chunk \"{:?}\"", chunk.grid_position);
				chunks.insert(chunk.grid_position, chunk);
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
