use crate::sector::config;
use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit};
use clap::Parser;
use env_logger::Env;
use futures::StreamExt;
use log::{error, info, warn};
use rayon::spawn_broadcast;
use sector::{Event, Sector};
use solarscape_backend_types::messages::AllowConnection;
use solarscape_shared::connection::{Connection, ServerEnd};
use sqlx::{postgres::PgConnectOptions, postgres::PgListener, PgPool};
use std::{
	collections::HashMap, env, fs::read_to_string, io, net::SocketAddr, path::PathBuf, sync::LazyLock, time::Instant,
};
use thiserror::Error;
use thread_priority::ThreadPriority;
use tokio::{io::AsyncReadExt, net::TcpListener, runtime::Runtime, select};

mod generation;
mod player;
mod sector;

#[derive(Parser)]
#[command(version)]
struct ClArgs {
	/// Postgres Connection Url, see: https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html
	#[arg(long)]
	postgres: PgConnectOptions,

	/// Socket address to accept connections on
	#[arg(long)]
	address: SocketAddr,

	/// Path to sector config file
	#[arg(long)]
	config: PathBuf,
}

fn main() -> Result<(), SectorServerError> {
	let start_time = Instant::now();

	let mut cl_args = ClArgs::parse();

	env_logger::init_from_env(Env::default().default_filter_or(if cfg!(debug_assertions) { "debug" } else { "info" }));

	info!("Solarscape (Server) v{}", env!("CARGO_PKG_VERSION"));

	let runtime = Runtime::new()?;
	let a = runtime.enter();

	cl_args.postgres = cl_args.postgres.application_name("solarscape-sector");
	let database = runtime.block_on(PgPool::connect_with(cl_args.postgres))?;

	let sector = {
		let config: config::Sector = {
			let string = read_to_string(cl_args.config)?;
			hocon::de::from_str(&string)?
		};

		Sector::new(database.clone(), config)
	};

	let shared_sector = sector.shared.clone();

	let mut allow_connection_listener = runtime.block_on(PgListener::connect_with(&database))?;
	runtime.block_on(allow_connection_listener.listen(&sector.name))?;
	let mut allow_connection_stream = allow_connection_listener.into_stream();

	let connection_listener = runtime.block_on(TcpListener::bind(cl_args.address))?;

	info!("Setting Rayon Thread Priority");
	spawn_broadcast(|_| {
		if let Err(error) = ThreadPriority::Min.set_for_current() {
			warn!("Failed to set Rayon Thread Priority to minimum: {error}")
		}
	});

	info!("Ready! {:.0?}", Instant::now() - start_time);

	runtime.spawn(async move {
		let mut key_id_map = HashMap::new();

		loop {
			select! {
				allow_connection = allow_connection_stream.next() => {
					let AllowConnection { id, key } = match allow_connection {
						None => {
							error!("allow connection stream closed?");
							return;
						}
						Some(allow_connection) => match allow_connection {
							Err(error) => {
								error!("error while reading allow_connection_notification: {error}");
								return;
							}
							Ok(allow_connection) => match serde_json::from_str(allow_connection.payload()) {
								Err(error) => {
									error!("error while deserializing allow connection notification: {error}");
									continue
								}
								Ok(allow_connection) => allow_connection,
							}
						}
					};

					key_id_map.insert(key, id);
				},

				connection = connection_listener.accept() => {
					let (mut stream, _) = match connection {
						Err(error) => {
							error!("unable to accept further connections due to error: {error}");
							return;
						},
						Ok(connection) => connection,
					};

					let length = match stream.read_u16_le().await {
						Ok(length) => length,
						_ => continue,
					};

					let mut buffer = vec![0; length as usize];
					match stream.read_exact(&mut buffer).await {
						Ok(_) => {},
						_ => continue,
					}

					let mut iterator = key_id_map.iter();
					while let Some((key, id)) = iterator.next() {
						let cipher = ChaCha20Poly1305::new(key.into());
						let version_data = match cipher.decrypt((&[0; 12]).into(), &*buffer) {
							Err(_) => continue,
							Ok(version_data) => version_data,
						};
						let (key, id) = (*key, *id);
						if version_data.len() == 4 && version_data == [0, 0, 0, 0] {
							let connection = Connection::<ServerEnd>::new(stream, cipher);
							key_id_map.remove(&key);
							shared_sector.send(Event::PlayerConnected(id, connection));
							break;
						}
					}
				}
			}
		}
	});

	sector.run();

	Ok(())
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum SectorServerError {
	Hocon(#[from] hocon::Error),
	Io(#[from] io::Error),
	Sqlx(#[from] sqlx::Error),
}
