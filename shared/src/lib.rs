mod clientbound;
mod serverbound;

pub mod data;
pub mod io;

use once_cell::sync::Lazy;

pub use clientbound::*;
pub use serverbound::*;

pub const PROTOCOL_VERSION: Lazy<u16> = Lazy::new(|| {
	env!("CARGO_PKG_VERSION_MAJOR")
		.parse()
		.expect("crate major version invalid")
});

pub const PACKET_LENGTH_LIMIT: usize = 2 ^ 16;
