use anyhow::Result;
use log::info;
use serde::Deserialize;
use solarscape_shared::world::SectorData;
use std::{
	env,
	fs::{self, DirEntry, File},
	io::Read,
	sync::Arc,
};

#[repr(transparent)]
pub struct Sector {
	shared: SectorData,
}

#[derive(Deserialize)]
struct SectorConfig {
	pub display_name: Box<str>,
}

impl Sector {
	pub fn load_all() -> Result<Vec<Arc<Sector>>> {
		let mut sectors = vec![];

		let mut sectors_path = env::current_dir()?;
		sectors_path.push("sectors");

		for path in fs::read_dir(sectors_path)? {
			if let Some(sector) = Sector::load(path?)? {
				sectors.push(sector);
			}
		}

		info!(
			"Loaded {} Sectors: {:?}",
			sectors.len(),
			sectors.iter().map(|sector| sector.display_name()).collect::<Vec<_>>()
		);

		Ok(sectors)
	}

	fn load(path: DirEntry) -> Result<Option<Arc<Sector>>> {
		let file_name = path.file_name();
		let name = file_name.to_string_lossy();

		if name.starts_with('.') || !path.metadata()?.is_dir() {
			return Ok(None);
		}

		let mut config_path = path.path();
		config_path.push("sector.conf");

		let mut file = File::open(config_path)?;
		let mut bytes = vec![];
		file.read_to_end(&mut bytes)?;
		let string = String::from_utf8(bytes)?;

		let configuration: SectorConfig = hocon::de::from_str(string.as_str())?;

		Ok(Some(Arc::new(Sector {
			shared: SectorData {
				name: name.into(),
				display_name: configuration.display_name,
			},
		})))
	}

	pub fn run(self: Arc<Self>) -> Result<()> {
		Ok(())
	}

	//noinspection RsNeedlessLifetimes
	pub fn shared<'a>(self: &'a Arc<Self>) -> &'a SectorData {
		&self.shared
	}

	//noinspection RsNeedlessLifetimes
	pub fn display_name<'a>(self: &'a Arc<Self>) -> &'a str {
		&self.shared.display_name
	}
}
