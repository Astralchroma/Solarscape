pub mod message {
	mod clientbound;
	mod serverbound;

	pub use clientbound::*;
	pub use serverbound::*;
}

pub mod connection;
pub mod triangulation_table;
pub mod types;
