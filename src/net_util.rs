use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use bytes::{BufMut, BytesMut};
use log::debug;
use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
};

use crate::{
    constant::{MAX_CONTROL_MESSAGE_SIZE, MESSAGE_LENGTH_SIZE_BYTES},
    error::{ClientError, Error, NetUtilError, Result, ServerError},
    message::{ClientEnvelope, ClientMessage, ServerEnvelope, ServerMessage},
};

pub async fn client_write_message<A>(stream: &mut A, message: ClientMessage) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ClientEnvelope::Message(message))
        .await
        .map_err(Error::ClientWriteError)
}

pub async fn client_write_error<A>(stream: &mut A, error: ClientError) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ClientEnvelope::Error(error))
        .await
        .map_err(Error::ClientWriteError)
}

pub async fn server_write_message<A>(stream: &mut A, message: ServerMessage) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ServerEnvelope::Message(message))
        .await
        .map_err(Error::ServerWriteError)
}

pub async fn server_write_error<A>(stream: &mut A, error: ServerError) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    write_control_message(stream, ServerEnvelope::Error(error))
        .await
        .map_err(Error::ServerWriteError)
}

pub async fn client_read_message<A>(stream: &mut A) -> Result<ServerMessage>
where
    A: AsyncReadExt + Unpin,
{
    match read_control_message::<_, ServerEnvelope>(stream)
        .await
        .map_err(Error::ClientReadError)?
    {
        ServerEnvelope::Message(message) => Ok(message),
        ServerEnvelope::Error(e) => Err(e.into()),
    }
}

pub async fn server_read_message<A>(stream: &mut A) -> Result<ClientMessage>
where
    A: AsyncReadExt + Unpin,
{
    match read_control_message::<_, ClientEnvelope>(stream)
        .await
        .map_err(Error::ServerReadError)?
    {
        ClientEnvelope::Message(message) => Ok(message),
        ClientEnvelope::Error(e) => Err(e.into()),
    }
}

async fn write_control_message<A, T>(
    stream: &mut A,
    message: T,
) -> std::result::Result<(), NetUtilError>
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
    stream.write_all(&buf[..]).await?;
    Ok(())
}

async fn read_control_message<A, T>(stream: &mut A) -> std::result::Result<T, NetUtilError>
where
    A: AsyncReadExt + Unpin,
    T: DeserializeOwned,
{
    let message_size = stream.read_u32().await?;

    if message_size > MAX_CONTROL_MESSAGE_SIZE {
        return Err(NetUtilError::ControlMessageTooLarge(message_size));
    }

    let mut buf = BytesMut::with_capacity(message_size as usize);
    let mut remaining_bytes: u64 = message_size as u64;
    let mut counter: usize = 0;
    while remaining_bytes > 0 {
        counter += 1;
        // Only read up-to the remaining-bytes, don't over read.
        // It's important that we don't read more as we don't want to mess up
        // the protocol. The next read should find the LENGTH as the first 4 bytes.
        let mut handle = stream.take(remaining_bytes);
        let bytes_read = handle.read_buf(&mut buf).await?;
        if bytes_read == 0 {
            // We have reached EOF. This is unexpected.
            // XXX: Handle
            return Err(NetUtilError::ConnectionWasClosedByPeer)
        }
        // usize is u64 in most cases.
        remaining_bytes -= bytes_read as u64;
    }
    debug!(
        "Received a control message ({} bytes) in {} iterations",
        message_size, counter
    );
    assert_eq!(message_size as usize, buf.len());

    let obj = serde_json::from_slice(&buf)?;
    Ok(obj)
}

pub trait TcpStreamExt {
    fn local_addr_string(&self) -> String;

    fn peer_addr_string(&self) -> String;
}

impl TcpStreamExt for TcpStream {
    fn local_addr_string(&self) -> String {
        match self.local_addr() {
            Ok(addr) => addr.to_string(),
            Err(e) => format!("<UNKNOWN:{e:?}>"),
        }
    }

    fn peer_addr_string(&self) -> String {
        match self.peer_addr() {
            Ok(addr) => addr.to_string(),
            Err(e) => format!("<UNKNOWN:{e:?}>"),
        }
    }
}

pub fn new_socket(bind_addr: Option<String>, prefer_ipv6: bool) -> Result<TcpSocket> {
    let socket_addr: SocketAddr = match bind_addr {
        Some(addr) => format!("{}:0", addr).parse()?,
        None => {
            if prefer_ipv6 {
                SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
            } else {
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
            }
        }
    };
    let socket = if socket_addr.is_ipv4() {
        TcpSocket::new_v4()
    } else {
        TcpSocket::new_v6()
    }?;
    socket.bind(socket_addr)?;

    Ok(socket)
}
