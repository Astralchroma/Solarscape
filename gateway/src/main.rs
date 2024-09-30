use argon2::Argon2;
use axum::{http::StatusCode, routing::get, Router};
use clap::Parser;
use endpoints::{web::get_create_account, web::get_htmx, web::get_root};
use env_logger::Env;
use log::info;
use sqlx::{postgres::PgConnectOptions, PgPool};
use std::{net::SocketAddr, sync::LazyLock, time::Instant};
use tokio::{net::TcpListener, runtime::Runtime};

mod types;

mod endpoints {
	pub mod web;
}

pub static ARGON_2: LazyLock<Argon2> = LazyLock::new(Argon2::default);

/// Gateway is Solarscape's API Server Implementation, responsible for Account Authentication.
#[derive(Parser)]
#[command(version)]
struct ClArgs {
	/// Postgres Connection Url, see: https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html
	#[arg(long)]
	postgres: PgConnectOptions,

	/// Socket address to accept connections on
	#[arg(long)]
	address: SocketAddr,
}

fn main() {
	let start_time = Instant::now();

	let mut cl_args = ClArgs::parse();

	env_logger::init_from_env(Env::default().default_filter_or(if cfg!(debug_assertions) { "debug" } else { "info" }));

	info!("Solarscape (Gateway) v{}", env!("CARGO_PKG_VERSION"));

	let runtime = Runtime::new().expect("failed to start tokio runtime");

	cl_args.postgres = cl_args.postgres.application_name("solarscape-gateway");
	let database = runtime
		.block_on(PgPool::connect_with(cl_args.postgres))
		.expect("failed to connect to PostgreSQL database");

	let router = Router::new()
		.route("/", get(get_root))
		.route("/htmx-2.0.2.min.js", get(get_htmx))
		.route("/create_account", get(get_create_account))
		.fallback(|| async { StatusCode::NOT_FOUND })
		.with_state(database);

	let listener = runtime
		.block_on(TcpListener::bind(cl_args.address))
		.expect("failed to bind to socket address");

	info!("Ready! {:.0?}", Instant::now() - start_time);

	runtime.block_on(async { axum::serve(listener, router).await }).unwrap();
}
