use crate::endpoints::{api, web};
use argon2::Argon2;
use axum::{http::StatusCode, Router};
use clap::Parser;
use env_logger::Env;
use itertools::Itertools;
use log::info;
use sqlx::{postgres::PgConnectOptions, PgPool};
use std::{net::SocketAddr, sync::Arc, sync::LazyLock, time::Instant};
use tokio::{net::TcpListener, runtime::Runtime};

mod extractors;
mod types;

mod endpoints {
	pub mod api;
	pub mod web;
}

pub static ARGON_2: LazyLock<Argon2> = LazyLock::new(Argon2::default);

#[derive(Parser)]
#[command(version)]
pub struct ClArgs {
	/// Postgres Connection Url, see: https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html
	#[arg(long)]
	pub postgres: PgConnectOptions,

	/// Socket address to accept connections on
	#[arg(long)]
	pub address: SocketAddr,

	/// Sector to log all players into
	#[arg(long)]
	pub sector: String,

	/// Address of sector to log all players into
	#[arg(long)]
	pub sector_address: String,
}

#[derive(Clone)]
pub struct Gateway {
	pub database: PgPool,
	pub cl_args: Arc<ClArgs>,
}

fn main() {
	let start_time = Instant::now();

	let mut cl_args = ClArgs::parse();

	env_logger::init_from_env(Env::default().default_filter_or(if cfg!(debug_assertions) { "debug" } else { "info" }));

	info!("Solarscape (Gateway) v{}", env!("CARGO_PKG_VERSION"));

	let runtime = Runtime::new().expect("failed to start tokio runtime");

	cl_args.postgres = cl_args.postgres.application_name("solarscape-gateway");
	let database = runtime
		.block_on(PgPool::connect_with(cl_args.postgres.clone()))
		.expect("failed to connect to PostgreSQL database");

	let listener = runtime
		.block_on(TcpListener::bind(cl_args.address))
		.expect("failed to bind to socket address");

	let router = Router::new()
		.nest("/web", web::router())
		.nest("/api", api::router())
		.fallback(|| async { StatusCode::NOT_FOUND })
		.with_state(Gateway {
			database,
			cl_args: Arc::new(cl_args),
		});

	info!("Ready! {:.0?}", Instant::now() - start_time);

	runtime.block_on(async { axum::serve(listener, router).await }).unwrap();
}

const LOOKUP: [char; 16] = [
	'0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

pub fn to_string(bytes: &[u8]) -> String {
	let mut string = String::with_capacity(32);
	for byte in bytes {
		string.push(LOOKUP[(byte >> 4) as usize]);
		string.push(LOOKUP[(byte & 0xF) as usize]);
	}

	string
}

// Not very good™️, but good enough, assumes lowercase, nonsensical bytes (not nibbles) are skipped
pub fn to_bytes(string: &str) -> Vec<u8> {
	let mut bytes = vec![];
	'bytes: for chars in &string.chars().chunks(2) {
		let chars: (char, char) = match chars.collect_tuple() {
			Some(value) => value,
			_ => break, // Simple truncate to avoid issues, we should handle this smarter later
		};

		let mut byte: u8 = 0;

		'nibble: {
			for (nibble, char) in LOOKUP.iter().enumerate() {
				if *char == chars.0 {
					byte += (nibble as u8) << 4;
					break 'nibble;
				}
			}

			continue 'bytes;
		}

		'nibble: {
			for (nibble, char) in LOOKUP.iter().enumerate() {
				if *char == chars.1 {
					byte += nibble as u8;
					break 'nibble;
				}
			}
			continue 'bytes;
		}

		bytes.push(byte);
	}

	bytes
}
