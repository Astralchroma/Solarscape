pub mod io;
pub mod protocol;
pub mod world;

use once_cell::sync::Lazy;

pub static PROTOCOL_VERSION: Lazy<u16> = Lazy::new(|| {
	env!("CARGO_PKG_VERSION_MAJOR")
		.parse()
		.expect("crate major version invalid")
});

pub const PACKET_LENGTH_LIMIT: usize = 1 << 13;
