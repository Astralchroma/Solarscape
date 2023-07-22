#![deny(clippy::unwrap_used)]

mod sector;
mod world;

use crate::world::World;
use anyhow::Result;
use log::info;
use solarscape_shared::{
	io::{PacketRead, PacketWrite},
	protocol::{Clientbound, Serverbound, PROTOCOL_VERSION},
	setup_logging,
};
use std::convert::Infallible;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<Infallible> {
	setup_logging();

	let world = World::new();

	let mut stream = TcpStream::connect("[::1]:23500").await?;
	info!("Connecting to [::1]:23500");

	stream
		.write_packet(&Serverbound::Hello {
			major_version: *PROTOCOL_VERSION,
		})
		.await?;

	loop {
		use Clientbound::*;

		match stream.read_packet().await? {
			Disconnected { reason } => panic!("Disconnected: {reason:?}"),
			SyncSector { name, display_name } => world.add_sector(name, display_name),
			ActiveSector { name } => world.set_active_sector(name),
			SyncChunk { .. } => {}
		}
	}
}
