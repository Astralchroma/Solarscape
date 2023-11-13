#![deny(clippy::unwrap_used)]

mod chunk;
mod configuration;
mod connection;
mod generator;
mod object;
mod sector;
mod server;
mod sync;

use crate::{
	configuration::Configuration, connection::ServerConnection, generator::SphereGenerator, object::Object,
	sector::Sector, server::Server, sync::Subscribers,
};
use anyhow::Result;
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

	let configuration = Configuration::load()?;
	let mut server = Server::default();

	for (sector_id, sector_configuration) in configuration.sectors {
		let sector = Sector {
			name: sector_id,
			display_name: sector_configuration.display_name,
		};

		let sector_entity = server.world.spawn((sector, Subscribers::new()));

		for object_configuration in sector_configuration.objects {
			let object = Object {
				sector: sector_entity,
				generator: Box::new(SphereGenerator {
					radius: object_configuration.radius,
				}),
			};

			let object_entity = server.world.spawn((object, Subscribers::new()));

			Object::generate_sphere(&mut server.world, object_entity)?;
		}
	}

	let (incoming_in, incoming) = mpsc::unbounded_channel();
	runtime.spawn(ServerConnection::r#await(incoming_in));

	server.run(incoming);
}
