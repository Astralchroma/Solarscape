#![deny(clippy::unwrap_used)]

use anyhow::Result;
use solarscape_shared::{
	io::{PacketRead, PacketWrite},
	protocol::{Clientbound, DisconnectReason::ProtocolViolation, Serverbound, PROTOCOL_VERSION},
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

	socket
		.write_packet(&Serverbound::Hello {
			major_version: *PROTOCOL_VERSION,
		})
		.await?;

	loop {
		match socket.read_packet().await? {
			Clientbound::Hello => break,
			Clientbound::Disconnected { reason: reason } => {
				eprintln!("Disconnected: {reason:?}");
				socket.shutdown().await?;
				return Ok(());
			}
			Clientbound::SyncSector { name, display_name } => {
				println!("Received sector \"{}\"", display_name);
				sectors.push(display_name);
			}
			Clientbound::ActiveSector {
				network_id: active_sector,
			} => {
				println!("Switched to sector \"{}\"", sectors[active_sector]);
				sector_id = active_sector;
			}
			Clientbound::SyncChunk { grid_position, data } => {
				println!("Received chunk \"{:?}\"", grid_position);
				chunks.insert(grid_position, data);
			}
			_ => {
				socket
					.write_packet(&Serverbound::Disconnected {
						reason: ProtocolViolation,
					})
					.await?;
				socket.shutdown().await?;
				return Ok(());
			}
		}
	}

	print!("Connected to [::1]:23500");

	loop {
		match socket.read_packet().await? {
			Clientbound::Disconnected { reason: reason } => {
				eprintln!("Disconnected: {reason:?}");
				socket.shutdown().await?;
				return Ok(());
			}
			Clientbound::SyncChunk { grid_position, data } => {
				println!("Received chunk \"{:?}\"", grid_position);
				chunks.insert(grid_position, data);
			}
			_ => {
				socket
					.write_packet(&Serverbound::Disconnected {
						reason: ProtocolViolation,
					})
					.await?;
				socket.shutdown().await?;
				return Ok(());
			}
		}
	}
}
