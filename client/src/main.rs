#![deny(clippy::unwrap_used)]

mod camera;
mod chunk;
mod client;
mod components;
mod connection;
mod renderer;
mod triangulation_table;

use anyhow::Result;
use clap::{Args, Parser};
use client::Client;
use native_dialog::{MessageDialog, MessageType};
use solarscape_shared::shared_main;
use std::{panic, path::PathBuf};

#[derive(Parser)]
pub struct Arguments {
	#[command(flatten)]
	backend: Backend,

	/// Enables debugging features
	#[arg(short, long)]
	debug: bool,

	/// Enables wgpu's tracing and outputs it to the specified location
	#[arg(long)]
	tracing: Option<PathBuf>,

	/// Disables vsync
	#[arg(long)]
	disable_vsync: bool,

	/// Specify an unofficial server to connect to
	#[arg(short, long)]
	server: Option<String>,
}

#[derive(Clone, Copy, Args)]
#[group(required = false, multiple = false)]
struct Backend {
	/// Forces using the OpenGL Backend for rendering
	#[arg(long)]
	gl: bool,

	/// Forces using the Vulkan Backend for rendering
	#[arg(long)]
	vulkan: bool,
}

fn handle_fatal_error(message: String) {
	let message = format!("Solarscape has encountered an unrecoverable error, details are below:\n{message}");

	eprintln!("{message}");

	let _ = MessageDialog::new()
		.set_title("Solarscape")
		.set_type(MessageType::Error)
		.set_text(&message)
		.show_alert();
}

fn main() {
	panic::set_hook(Box::new(|panic| handle_fatal_error(panic.to_string())));

	match _main() {
		Ok(_) => {}
		Err(error) => handle_fatal_error(error.to_string()),
	}
}

fn _main() -> Result<()> {
	let arguments = Arguments::parse();
	let runtime = shared_main()?;

	Ok(Client::run(arguments, runtime)?)
}
