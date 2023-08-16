#![deny(clippy::unwrap_used)]

mod chunk;
mod client;
mod object;
mod orbit_camera;
mod sector;
mod triangulation_table;
mod world;

use anyhow::Result;
use client::Client;
use solarscape_shared::shared_main;
use std::convert::Infallible;

fn main() -> Result<Infallible> {
	let runtime = shared_main()?;

	Client::run(runtime)
}
