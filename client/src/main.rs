#![warn(clippy::nursery)]

use crate::client::Client;
use env_logger::Env;
use log::info;
use std::{env, error::Error, time::Instant};
use tokio::runtime::Builder;
use winit::event_loop::EventLoop;

mod client;
mod connection;
mod player;
mod world;

fn main() -> Result<(), Box<dyn Error>> {
	let start_time = Instant::now();

	env_logger::init_from_env(Env::default().default_filter_or(if cfg!(debug) {
		"solarscape_client=debug"
	} else {
		"solarscape_client=info"
	}));

	info!("Solarscape (Client) v{}", env!("CARGO_PKG_VERSION"));

	info!("Command Line: {:?}", env::args().collect::<Vec<_>>().join(" "));
	info!("Working Directory: {:?}", env::current_dir()?);

	let name = env::args().nth(1).expect("name").into_boxed_str();

	let sector_endpoint = env::args()
		.nth(2)
		.unwrap_or_else(|| String::from("ws://localhost:8000/example"))
		.into_boxed_str();

	info!("Setting name to {name:?}");

	let runtime = Builder::new_multi_thread()
		.thread_name("io-worker")
		.worker_threads(1)
		.enable_io()
		.enable_time()
		.build()?;

	let _guard = runtime.enter();

	info!("Started Async Runtime with 1 worker thread");

	let event_loop = EventLoop::with_user_event().build()?;
	let mut client = Client {
		name,
		sector_endpoint,
		event_loop_proxy: event_loop.create_proxy(),
		state: None,
	};

	info!("Event loop ready in {:.0?}", Instant::now() - start_time);

	event_loop.run_app(&mut client)?;

	Ok(())
}
