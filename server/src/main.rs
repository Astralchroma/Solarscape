#![deny(clippy::unwrap_used)]

mod configuration;
mod connection;
mod generator;
mod player;
mod server;
mod sync;
mod voxel_object;

use crate::generator::{ArcGenerator, SphereGenerator};
use crate::{configuration::Configuration, connection::ServerConnection, server::Server, sync::Subscribers};
use anyhow::Result;
use hecs::World;
use solarscape_shared::{components::Location, components::Sector, components::VoxelObject, shared_main};
use std::{collections::HashMap, convert::Infallible, env, fs};
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

		for voxel_object_configuration in sector_configuration.voxel_objects {
			world.spawn((
				VoxelObject { sector: sector_entity },
				Location {
					position: voxel_object_configuration.position.into(),
					rotation: voxel_object_configuration.rotation.into(),
					scale: 1.0,
				},
				ArcGenerator::new(SphereGenerator {
					radius: voxel_object_configuration.radius,
				}),
				Subscribers::new(),
			));
		}
	}

	let (incoming_in, incoming) = mpsc::unbounded_channel();
	runtime.spawn(ServerConnection::r#await(incoming_in));

	let server = Server {
		runtime,
		default_sector: default_sector.expect("a default sector is required"),

		world,

		next_connection_id: 0,
		connections: HashMap::new(),
	};

	server.run(incoming)
}
