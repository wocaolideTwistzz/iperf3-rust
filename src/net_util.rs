use bytes::{BufMut, BytesMut};
use log::debug;
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    constant::{MAX_CONTROL_MESSAGE_SIZE, MESSAGE_LENGTH_SIZE_BYTES},
    error::{ClientError, Error, Result, ServerError},
    message::{ClientEnvelope, ClientMessage, ServerEnvelope, ServerMessage},
};

pub async fn client_write_message<A>(stream: &mut A, message: ClientMessage) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ClientEnvelope::Message(message)).await
}

pub async fn client_write_error<A>(stream: &mut A, error: ClientError) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ClientEnvelope::Error(error)).await
}

pub async fn server_write_message<A>(stream: &mut A, message: ServerMessage) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ServerEnvelope::Message(message)).await
}

pub async fn server_write_error<A>(stream: &mut A, error: ServerError) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ServerEnvelope::Error(error)).await
}

pub async fn client_read_message<A>(stream: &mut A) -> Result<ServerMessage>
where
    A: AsyncReadExt + Unpin,
{
    match read_control_message::<_, ServerEnvelope>(stream).await? {
        ServerEnvelope::Message(message) => Ok(message),
        ServerEnvelope::Error(e) => Err(e.into()),
    }
}

pub async fn server_read_message<A>(stream: &mut A) -> Result<ClientMessage>
where
    A: AsyncReadExt + Unpin,
{
    match read_control_message::<_, ClientEnvelope>(stream).await? {
        ClientEnvelope::Message(message) => Ok(message),
        ClientEnvelope::Error(e) => Err(e.into()),
    }
}

async fn write_control_message<A, T>(stream: &mut A, message: T) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
    T: Serialize,
{
    let payload = serde_json::to_vec(&message)?;

    assert!(payload.len() <= (u32::MAX) as usize);

    let mut buf = BytesMut::with_capacity(MESSAGE_LENGTH_SIZE_BYTES + payload.len());
    buf.put_u32(payload.len() as u32);
    buf.put_slice(&payload);

    debug!("Sent: {} bytes", buf.len());
    stream.write_all(&buf).await?;
    Ok(())
}

async fn read_control_message<A, T>(stream: &mut A) -> Result<T>
where
    A: AsyncReadExt + Unpin,
    T: DeserializeOwned,
{
    let message_size = stream.read_u32().await?;

    if message_size > MAX_CONTROL_MESSAGE_SIZE {
        return Err(Error::ControlMessageTooLarge(message_size));
    }

    let mut buf = BytesMut::with_capacity(message_size as usize);
    stream.read_exact(&mut buf).await?;

    debug!("Received: {} bytes", buf.len());

    Ok(serde_json::from_slice(&buf)?)
}
