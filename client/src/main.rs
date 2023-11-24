#![deny(clippy::unwrap_used)]

mod chunk;
mod client;
mod component;
mod connection;
mod orbit_camera;
mod triangulation_table;

use anyhow::Result;
use client::Client;
use solarscape_shared::shared_main;

fn main() -> Result<()> {
	let runtime = shared_main()?;

	Client::run(runtime)
}
