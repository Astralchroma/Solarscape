pub mod connection;
pub mod server;

use crate::server::Server;
use anyhow::Result;
use std::{env, panic, process::exit};

#[tokio::main]
async fn main() -> Result<()> {
	// If there is a panic we should always exit immediately, tokio won't do this for us.
	let default_panic = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		default_panic(info);
		exit(1);
	}));

	let mut cargo = env::current_dir()?;
	cargo.push("Cargo.toml");

	// if Cargo.toml exists, assume we are running in a development environment.
	if cargo.exists() {
		let mut data = env::current_dir()?;
		data.push("server");
		data.push("run");

		env::set_current_dir(data)?;
	}

	Server::new().await_connections().await
}
