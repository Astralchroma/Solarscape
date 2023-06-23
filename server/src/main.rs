pub mod connection;
pub mod server;

use crate::server::Server;
use anyhow::Result;
use std::{env, fs, panic};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
	let token = CancellationToken::new();

	// If a Tokio task panics, Tokio will catch the panic with the intent that the caller will handle it.
	// However, generally you shouldn't try to recover a panic, instead your goal should be to safely exit.
	let hook_token = token.clone();
	let default_panic = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		default_panic(info);
		hook_token.cancel();
	}));

	let mut cargo = env::current_dir()?;
	cargo.push("Cargo.toml");

	// if Cargo.toml exists, assume we are running in a development environment.
	if cargo.exists() {
		let mut data = env::current_dir()?;
		data.push("server");
		data.push("run");

		fs::create_dir_all(data.clone())?;
		env::set_current_dir(data)?;
	}

	let server_token = token.clone();
	tokio::spawn(async move {
		let result = Server::new().await_connections().await;
		server_token.cancel();
		if let Err(error) = result {
			eprintln!("{error:?}")
		}
	});

	token.cancelled_owned().await;
	Ok(())
}
