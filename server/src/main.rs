#![deny(clippy::unwrap_used)]

mod chunk;
mod connection;
mod object;
mod sector;
mod server;
mod sync;

use crate::{connection::ServerConnection, object::Object, sector::Sector, server::Server, sync::Subscribers};
use anyhow::Result;
use hecs::With;
use solarscape_shared::shared_main;
use std::{convert::Infallible, env, fs};
use tokio::sync::mpsc;

fn main() -> Result<Infallible> {
	let runtime = shared_main()?;

	// Avoid altering project files if running in Cargo
	if env::var("CARGO").is_ok() {
		let mut working_directory = env::current_dir()?;
		working_directory.push("server/run");

		fs::create_dir_all(&working_directory)?;
		env::set_current_dir(working_directory)?;
	}

	let mut server = Server::default();

	server.world.spawn_batch(Sector::load_all()?);

	let objects = server
		.world
		.query::<With<(), &Sector>>()
		.into_iter()
		.map(|(sector, _)| (Object { sector }, Subscribers::new()))
		.collect::<Vec<_>>();

	let objects = server.world.spawn_batch(objects).collect::<Vec<_>>();

	for object in objects {
		Object::generate_sphere(&mut server.world, object)
	}

	let (incoming_in, incoming) = mpsc::unbounded_channel();
	runtime.spawn(ServerConnection::r#await(incoming_in));

	server.run(incoming);
}
