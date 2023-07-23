#![deny(clippy::unwrap_used)]

mod chunk;
mod connection;
mod object;
mod sector;
mod world;

use crate::{connection::Connection, world::World};
use anyhow::Result;
use log::info;
use solarscape_shared::shared_main;
use std::{convert::Infallible, env, fs, sync::Arc};
use tokio::net::TcpListener;

fn main() -> Result<Infallible> {
	let runtime = shared_main()?;

	let mut cargo = env::current_dir()?;
	cargo.push("Cargo.toml");

	// if Cargo.toml exists, assume we are running in a development environment.
	if cargo.exists() {
		let mut data = env::current_dir()?;
		data.push("server");
		data.push("run");

		fs::create_dir_all(data.clone())?;
		env::set_current_dir(data)?;
	}

	let world = World::new()?;

	runtime.block_on(handle_connections(world))
}

async fn handle_connections(world: Arc<World>) -> Result<Infallible> {
	let socket = TcpListener::bind("[::]:23500").await?;
	info!("Listening on [::]:23500");

	loop {
		let (stream, address) = socket.accept().await?;
		tokio::spawn(Connection::accept(world.clone(), stream, address));
	}
}
