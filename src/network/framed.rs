use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

pub const MAX_WIRE_FRAME_SIZE: usize = 65535;

pub struct FramedWriter {
    writer: OwnedWriteHalf,
}

impl FramedWriter {
    pub fn new(writer: OwnedWriteHalf) -> Self {
        Self { writer }
    }

    pub async fn write_frame(&mut self, payload: &[u8]) -> std::io::Result<()> {
        if payload.len() > MAX_WIRE_FRAME_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Frame exceeds maximum wire size",
            ));
        }

        let len = payload.len() as u32;
        self.writer.write_u32(len).await?;
        self.writer.write_all(payload).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

pub struct FramedReader {
    reader: OwnedReadHalf,
}

impl FramedReader {
    pub fn new(reader: OwnedReadHalf) -> Self {
        Self { reader }
    }

    pub async fn read_frame(&mut self) -> std::io::Result<Vec<u8>> {
        let len = self.reader.read_u32().await? as usize;
        if len > MAX_WIRE_FRAME_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Frame exceeds maximum wire size",
            ));
        }

        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf).await?;
        Ok(buf)
    }
}
