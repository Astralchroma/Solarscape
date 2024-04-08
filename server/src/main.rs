#![warn(clippy::nursery)]

mod connection;
mod generation;
mod player;
mod world;

use crate::{connection::Connection, world::Sector};
use axum::{http::StatusCode, routing::get};
use log::{info, warn, LevelFilter::Trace};
use std::{env, fs, io, sync::Arc, sync::Barrier, thread, time::Instant};
use tokio::sync::mpsc::{unbounded_channel as channel, UnboundedSender as Sender};
use tokio::{net::TcpListener, runtime::Builder};

type Sectors = Arc<dashmap::DashMap<String, Sender<Connection>>>;

fn main() -> io::Result<()> {
	let start_time = Instant::now();

	log::set_logger(&solarscape_shared::StdLogger).expect("logger must not already be set");
	log::set_max_level(Trace);

	info!("Solarscape (Server) v{}", env!("CARGO_PKG_VERSION"));

	if env::var_os("CARGO").is_some() {
		warn!("Running in development environment! Changing working directory to avoid contaminating repository");
		let mut working_directory = env::current_dir()?;
		working_directory.push("server/run");
		fs::create_dir_all(&working_directory)?;
		env::set_current_dir(working_directory)?;
	}

	let server_name = env::var("SOLARSCAPE_SERVER_NAME").expect("SOLARSCAPE_SERVER_NAME must be set and valid");

	let static_sectors: Vec<String> = fs::read_to_string(format!("{server_name}.sectors"))?
		.split_whitespace()
		.map(String::from)
		.collect();

	info!("Command Line: {:?}", env::args().collect::<Vec<_>>().join(" "));
	info!("Working Directory: {:?}", env::current_dir()?);
	info!("Server Name: {server_name:?}");
	info!("Static Sectors: {static_sectors:?}");

	let runtime = Builder::new_multi_thread()
		.thread_name("io-worker")
		.worker_threads(1)
		.enable_io()
		.enable_time()
		.build()?;

	info!("Started Async Runtime with 1 worker thread");
	info!("Loading sectors");

	let sectors = Sectors::default();

	let barrier = Arc::new(Barrier::new(static_sectors.len() + 1));
	for sector_name in static_sectors {
		let (send, receiver) = channel();
		sectors.insert(sector_name.clone(), send);

		let barrier = barrier.clone();
		thread::Builder::new().name(sector_name.clone()).spawn(move || {
			let start_time = Instant::now();

			let sector = Sector::load();

			let end_time = Instant::now();
			let load_time = end_time - start_time;
			info!("{sector_name:?} ready! {load_time:.0?}");

			barrier.wait();

			sector.run(receiver);
		})?;
	}

	barrier.wait();

	let router = axum::Router::new()
		.route("/:sector", get(Connection::await_connections))
		.fallback(|| async { StatusCode::NOT_FOUND })
		.with_state(sectors);

	let end_time = Instant::now();
	let load_time = end_time - start_time;
	info!("Ready! {load_time:.0?}");

	runtime.block_on(async {
		let listener = TcpListener::bind("[::]:8000").await?;

		axum::serve(listener, router).await?;
		Ok::<(), io::Error>(())
	})
}
