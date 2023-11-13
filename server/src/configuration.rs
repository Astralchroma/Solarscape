//! This doesnt need to be its own module right now, but it is expected to get a lot bigger in the future.

use serde::Deserialize;
use std::{collections::HashMap, fs::File, io, io::Read, str, str::Utf8Error};
use thiserror::Error;

#[derive(Deserialize)]
pub struct Configuration {
	pub default_sector: Box<str>,

	pub sectors: HashMap<Box<str>, SectorConfiguration>,
}

#[derive(Deserialize)]
pub struct SectorConfiguration {
	pub display_name: Box<str>,

	pub objects: Vec<ObjectConfiguration>,
}

#[derive(Deserialize)]
pub struct ObjectConfiguration {
	pub radius: f32,
}

impl Configuration {
	pub fn load() -> Result<Configuration, ConfigurationLoadError> {
		let mut file = File::open("server.conf")?;
		let length = file.metadata()?.len() as usize;
		let mut buffer = vec![0; length];
		file.read_exact(&mut buffer)?;
		Ok(hocon::de::from_str(str::from_utf8(&buffer)?)?)
	}
}

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ConfigurationLoadError {
	Io(#[from] io::Error),
	Utf8(#[from] Utf8Error),
	Parse(#[from] hocon::Error),
}
