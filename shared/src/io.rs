use crate::PACKET_LENGTH_LIMIT;
use anyhow::anyhow;
use async_trait::async_trait;
use bincode::{config::standard, Decode, Encode};
use integer_encoding::{VarIntAsyncReader, VarIntAsyncWriter};
use tokio::{
	io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
	net::{
		tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf},
		TcpStream,
	},
};

#[async_trait]
pub trait PacketWrite: AsyncWrite + Unpin + Send {
	async fn write_packet<T: Encode + Sync>(&mut self, data: &T) -> anyhow::Result<()> {
		let buffer = bincode::encode_to_vec(data, standard())?;
		self.write_varint_async(buffer.len()).await?;
		self.write_all(&buffer).await?;
		Ok(())
	}
}

impl PacketWrite for OwnedWriteHalf {}

impl<'a> PacketWrite for WriteHalf<'a> {}

impl PacketWrite for TcpStream {}

#[async_trait]
pub trait PacketRead: AsyncRead + Unpin + Send {
	async fn read_packet<T: Decode>(&mut self) -> anyhow::Result<T> {
		let length = self.read_varint_async().await?;
		if length > PACKET_LENGTH_LIMIT {
			return Err(anyhow!("packet oversized: {length}"));
		}
		let mut buffer = vec![0; length];
		self.read_exact(&mut buffer).await?;
		Ok(bincode::decode_from_slice(&buffer, standard())?.0)
	}
}

impl PacketRead for OwnedReadHalf {}

impl<'a> PacketRead for ReadHalf<'a> {}

impl PacketRead for TcpStream {}
