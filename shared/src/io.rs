use crate::protocol::PACKET_LENGTH_LIMIT;
use anyhow::Result;
use async_trait::async_trait;
use bincode::{
	config::standard,
	error::{DecodeError, EncodeError},
	Decode, Encode,
};
use std::io::{self, Error, ErrorKind};
use thiserror::Error;
use tokio::{
	io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
	net::TcpStream,
};

#[async_trait]
pub trait PacketWrite: AsyncWrite + Unpin + Send {
	async fn write_packet<T: Encode + Sync>(&mut self, data: &T) -> Result<(), ConnectionError> {
		let buffer = bincode::encode_to_vec(data, standard())?;
		self.write_u16(buffer.len() as u16).await?;
		self.write_all(&buffer).await?;
		Ok(())
	}
}

impl PacketWrite for TcpStream {}

#[async_trait]
pub trait PacketRead: AsyncRead + Unpin + Send {
	async fn read_packet<T: Decode>(&mut self) -> Result<T, ConnectionError> {
		let length = self.read_u16().await? as usize;
		if length > PACKET_LENGTH_LIMIT {
			return Err(ConnectionError::OversizedPacket);
		}
		let mut buffer = vec![0; length];
		self.read_exact(&mut buffer).await?;
		Ok(bincode::decode_from_slice(&buffer, standard())?.0)
	}
}

impl PacketRead for TcpStream {}

#[derive(Debug, Error)]
pub enum ConnectionError {
	#[error(transparent)]
	DecodeError(#[from] DecodeError),

	#[error(transparent)]
	EncodeError(#[from] EncodeError),

	#[error("packet exceeded protocol size limit")]
	OversizedPacket,

	#[error("peer disconnected unexpectedly")]
	UnexpectedDisconnect,

	#[error(transparent)]
	Unhandled(#[from] anyhow::Error),
}

impl From<io::Error> for ConnectionError {
	fn from(value: Error) -> Self {
		match value.kind() {
			ErrorKind::UnexpectedEof => ConnectionError::UnexpectedDisconnect,
			_ => ConnectionError::Unhandled(value.into()),
		}
	}
}
