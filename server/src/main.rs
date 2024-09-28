#![warn(clippy::nursery)]
#![allow(clippy::option_if_let_else)] // Looks dumb

mod generation;
mod player;
mod sector;

use crate::{config::Config, player::Connection, sector::Sector, sector::SectorHandle};
use axum::{http::StatusCode, routing::get};
use env_logger::Env;
use log::{error, info, warn};
use rayon::spawn_broadcast;
use std::sync::{Arc, Barrier};
use std::{collections::HashMap, env, fs, fs::File, io, io::Read, net::SocketAddr, path::PathBuf, time::Instant};
use thread_priority::ThreadPriority;
use tokio::{net::TcpListener, runtime::Builder};

type Sectors = Arc<HashMap<Box<str>, Arc<SectorHandle>>>;

mod config {
	use serde::Deserialize;

	#[derive(Deserialize)]
	pub struct Config {
		pub name: Box<str>,
		pub sectors: Vec<Sector>,
	}

	#[derive(Deserialize)]
	pub struct Sector {
		pub name: Box<str>,
		pub voxjects: Vec<Voxject>,
	}

	#[derive(Deserialize)]
	pub struct Voxject {
		pub name: Box<str>,
	}
}

fn main() -> io::Result<()> {
	let start_time = Instant::now();

	env_logger::init_from_env(Env::default().default_filter_or(if cfg!(debug) {
		"solarscape_server=debug"
	} else {
		"solarscape_server=info"
	}));

	info!("Solarscape (Server) v{}", env!("CARGO_PKG_VERSION"));
	info!("Command Line: {:?}", env::args().collect::<Vec<_>>().join(" "));

	if env::var_os("CARGO").is_some() {
		warn!("Running in development environment! Changing working directory to avoid contaminating repository");
		let mut working_directory = env::current_dir()?;
		working_directory.push("server/run");
		fs::create_dir_all(&working_directory)?;
		env::set_current_dir(working_directory)?;
	}

	info!("Working Directory: {:?}", env::current_dir()?);

	let Config { name, sectors }: Config = {
		let path: PathBuf = env::var("SOLARSCAPE_CONFIG")
			.expect("environment variable 'SOLARSCAPE_CONFIG' must be set")
			.into();

		info!("Configuration File: {path:?}");

		let mut string = String::new();

		File::open(path)
			.expect("configuration file must exist")
			.read_to_string(&mut string)
			.expect("configuration file must be readable");

		hocon::de::from_str(&string).expect("configuration file must be valid")
	};

	info!("Server Name: {:?}", name);

	let runtime = Arc::new(
		Builder::new_multi_thread()
			.thread_name("io-worker")
			.worker_threads(1)
			.enable_io()
			.enable_time()
			.build()?,
	);

	info!("Started Async Runtime with 1 worker thread");

	info!("Setting Rayon Thread Priority");
	spawn_broadcast(|_| {
		if let Err(error) = ThreadPriority::Min.set_for_current() {
			error!("Failed to set Rayon Thread Priority to minimum: {error}")
		}
	});

	info!("Loading sectors");

	let barrier = Arc::new(Barrier::new(sectors.len() + 1));
	let sectors = Arc::new(
		sectors
			.into_iter()
			.map(|config| {
				let barrier = barrier.clone();
				let sector = Sector::load(config, move || {
					barrier.wait();
				});
				(sector.name.clone(), sector)
			})
			.collect(),
	);

	barrier.wait();

	let router = axum::Router::new()
		.route("/:sector", get(Connection::connect))
		.fallback(|| async { StatusCode::NOT_FOUND })
		.with_state(sectors);

	let end_time = Instant::now();
	let load_time = end_time - start_time;
	info!("Ready! {load_time:.0?}");

	runtime.block_on(async {
		let listener = TcpListener::bind("[::]:8000").await?;

		axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await?;
		Ok::<(), io::Error>(())
	})
}
