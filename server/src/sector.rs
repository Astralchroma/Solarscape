use anyhow::Result;
use serde::Deserialize;
use solarscape_shared::data::SectorMeta;
use std::borrow::Cow;
use std::{env, fs::File, io::Read, sync::Arc};

#[repr(transparent)]
pub struct Sector {
	meta: SectorMeta,
}

#[derive(Deserialize)]
struct Configuration {
	pub display_name: String,
}

impl Sector {
	pub fn load(key: &str) -> Result<Arc<Sector>> {
		let mut sector_path = env::current_dir()?;
		sector_path.push("sectors");
		sector_path.push(key);

		let mut configuration_path = sector_path.clone();
		configuration_path.push("sector.conf");

		let mut file = File::open(configuration_path)?;
		let mut bytes = vec![];
		file.read_to_end(&mut bytes)?;
		let string = String::from_utf8(bytes)?;

		let configuration: Configuration = hocon::de::from_str(string.as_str())?;

		Ok(Arc::new(Sector {
			meta: SectorMeta {
				name: Arc::from(key),
				display_name: Arc::from(configuration.display_name),
			},
		}))
	}

	pub fn run(self: Arc<Self>) -> Result<()> {
		Ok(())
	}

	#[allow(clippy::needless_lifetimes)]
	pub fn meta<'a>(self: &'a Arc<Self>) -> Cow<'a, SectorMeta> {
		Cow::Borrowed(&self.meta)
	}

	pub fn display_name(self: &Arc<Self>) -> Arc<str> {
		self.meta.display_name.clone()
	}
}
