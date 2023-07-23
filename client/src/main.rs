#![deny(clippy::unwrap_used)]

mod chunk;
mod object;
mod sector;
mod world;

use crate::{chunk::Chunk, object::Object, world::World};
use anyhow::Result;
use log::info;
use solarscape_shared::{
	io::{PacketRead, PacketWrite},
	protocol::{Clientbound, Serverbound, PROTOCOL_VERSION},
	shared_main,
};
use std::{convert::Infallible, sync::Arc};
use tokio::net::TcpStream;

fn main() -> Result<Infallible> {
	let runtime = shared_main()?;

	let world = World::new();

	runtime.block_on(handle_connection(world))
}

async fn handle_connection(world: Arc<World>) -> Result<Infallible> {
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
			AddObject { object_id } => {
				info!("Added object {object_id}");

				world
					.active_sector()
					.objects
					.borrow_mut()
					.insert(object_id, Object::new(object_id));
			}
			SyncChunk {
				object_id,
				grid_position,
				data,
			} => {
				info!("Added chunk {grid_position:?} to {object_id}");

				let chunk = Chunk { grid_position, data };
				world
					.active_sector()
					.objects
					.borrow()
					.get(&object_id)
					.expect("object_id of chunk should exist")
					.chunks
					.borrow_mut()
					.insert(grid_position, chunk);
			}
		}
	}
}
