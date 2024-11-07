#[cfg(feature = "world")]
pub mod connection;

pub mod data;

#[cfg(feature = "world")]
pub mod structure;

pub mod message {
	#[cfg(feature = "backend")]
	pub mod backend;

	#[cfg(feature = "world")]
	pub mod clientbound;

	#[cfg(feature = "world")]
	pub mod serverbound;
}

#[cfg(feature = "world")]
pub mod triangulation_table;
