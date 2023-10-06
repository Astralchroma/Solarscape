#![deny(clippy::unwrap_used)]

mod chunk;
mod connection;
mod object;
mod sector;
mod server;
mod sync;

use crate::{connection::Connection, object::Object, sector::Sector, server::Server};
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
		.map(|(entity, _)| (Object::sphere(entity),))
		.collect::<Vec<_>>();

	server.world.spawn_batch(objects);

	let (incoming_in, incoming) = mpsc::unbounded_channel();
	runtime.spawn(Connection::r#await(incoming_in));

	server.run(incoming);
}
