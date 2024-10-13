use crate::client::Client;
use clap::Parser;
use env_logger::Env;
use log::info;
use reqwest::Url;
use std::{env, error::Error, time::Instant};
use tokio::runtime::Runtime;
use winit::event_loop::EventLoop;

mod client;
mod player;
mod world;

#[derive(Parser)]
#[command(version)]
pub struct ClArgs {
	/// Solarscape Gateway API Endpoint
	#[arg(long, default_value = "https://solarscape.astralchroma.dev/api")]
	api_endpoint: Url,

	/// Email Address to log in with
	#[arg(long)]
	email: String,

	/// Password to log in with
	#[arg(long)]
	password: String,
}

fn main() -> Result<(), Box<dyn Error>> {
	let start_time = Instant::now();

	let cl_args = ClArgs::parse();

	env_logger::init_from_env(Env::default().default_filter_or(if cfg!(debug_assertions) {
		"solarscape_client=debug"
	} else {
		"solarscape_client=info"
	}));

	info!("Solarscape (Client) v{}", env!("CARGO_PKG_VERSION"));

	let runtime = Runtime::new()?;
	let _guard = runtime.enter();

	let event_loop = EventLoop::with_user_event().build()?;
	let mut client = Client { cl_args, state: None };

	info!("Event loop ready in {:.0?}", Instant::now() - start_time);

	event_loop.run_app(&mut client)?;

	Ok(())
}
