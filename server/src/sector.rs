use crate::connection::Connection;
use crate::sync::{Subscribers, Syncable};
use hecs::Entity;
use log::{error, info};
use serde::Deserialize;
use solarscape_shared::protocol::Clientbound;
use std::{env, fs, fs::DirEntry, fs::File, io, io::Read};
use thiserror::Error;

pub struct Sector {
	pub name: Box<str>,
	pub display_name: Box<str>,
}

#[derive(Deserialize)]
struct SectorConfig {
	pub display_name: Box<str>,
}

impl Sector {
	pub fn load_all() -> Result<Vec<(Sector, Subscribers)>, SectorLoadError> {
		let mut sector_path = env::current_dir()?;
		sector_path.push("sectors");

		fs::read_dir(sector_path)?
			.filter_map(Result::ok)
			.filter(|path| path.metadata().is_ok_and(|path| path.is_dir()))
			.map(Sector::load)
			.collect()
	}

	fn load(path: DirEntry) -> Result<(Sector, Subscribers), SectorLoadError> {
		let file_name = path.file_name();
		let name = file_name.to_string_lossy();

		let mut config_path = path.path();
		config_path.push("sector.conf");

		let mut file = File::open(config_path)?;
		let mut string = String::new();
		file.read_to_string(&mut string)?;

		let configuration: SectorConfig = hocon::de::from_str(string.as_str())?;

		let sector = Sector {
			name: name.into(),
			display_name: configuration.display_name,
		};

		info!("[{}] Sector Loaded", sector.display_name);

		Ok((sector, Subscribers::new()))
	}
}

impl Syncable for Sector {
	fn sync(&self, entity: Entity, connection: &mut Connection) {
		connection.send(Clientbound::SyncSector {
			entity_id: entity.to_bits().get(),
			name: self.name.clone(),
			display_name: self.display_name.clone(),
		})
	}
}

#[derive(Debug, Error)]
pub enum SectorLoadError {
	#[error(transparent)]
	IoError(#[from] io::Error),

	#[error(transparent)]
	ParseError(#[from] hocon::Error),
}
