#![deny(clippy::unwrap_used)]

mod chunk;
mod configuration;
mod connection;
mod generator;
mod object;
mod server;
mod sync;

use crate::generator::{BoxedGenerator, SphereGenerator};
use crate::{
	configuration::Configuration, connection::ServerConnection, object::generate_sphere, server::Server,
	sync::Subscribers,
};
use anyhow::Result;
use hecs::World;
use solarscape_shared::{component::Location, component::Object, component::Sector, shared_main};
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
			let object_entity = world.spawn((
				Object { sector: sector_entity },
				Location {
					position: object_configuration.position.into(),
					rotation: object_configuration.rotation.into(),
					scale: 1.0,
				},
				BoxedGenerator::new(SphereGenerator {
					radius: object_configuration.radius,
				}),
				Subscribers::new(),
			));

			generate_sphere(&mut world, object_entity)?;
		}
	}

	let server = Server {
		default_sector: default_sector.expect("a default sector is required"),

		world,
	};

	let (incoming_in, incoming) = mpsc::unbounded_channel();
	runtime.spawn(ServerConnection::r#await(incoming_in));

	server.run(incoming)
}
