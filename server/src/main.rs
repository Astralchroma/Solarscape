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
use hecs::World;
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

	let mut world = World::new();
	let mut default_sector = None;

	for (sector_id, sector_configuration) in configuration.sectors {
		let sector = Sector {
			name: sector_id.clone(),
			display_name: sector_configuration.display_name,
		};

		let sector_entity = world.spawn((sector, Subscribers::new()));

		if sector_id == configuration.default_sector {
			default_sector = Some(sector_entity);
		}

		for object_configuration in sector_configuration.objects {
			let object = Object {
				sector: sector_entity,
				generator: Box::new(SphereGenerator {
					radius: object_configuration.radius,
				}),
			};

			let object_entity = world.spawn((object, Subscribers::new()));

			Object::generate_sphere(&mut world, object_entity)?;
		}
	}

	let server = Server {
		default_sector: default_sector.expect("a default sector is required"),

		world,
	};

	let (incoming_in, incoming) = mpsc::unbounded_channel();
	runtime.spawn(ServerConnection::r#await(incoming_in));

	server.run(incoming);
}
