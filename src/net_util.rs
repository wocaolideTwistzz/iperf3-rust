use bytes::{BufMut, BytesMut};
use log::debug;
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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

use libc::{__u8, __u16, __u32, __u64};

#[repr(C)]
#[derive(Default)]
pub struct TcpInfo {
    pub tcpi_state: __u8,
    pub tcpi_ca_state: __u8,
    pub tcpi_retransmits: __u8,
    pub tcpi_probes: __u8,
    pub tcpi_backoff: __u8,
    pub tcpi_options: __u8,
    pub tcpi_snd_wscale_rcv_wscale: __u8,
    pub tcpi_delivery_rate_app_limited_fastopen_client_fail: __u8,

    pub tcpi_rto: __u32,
    pub tcpi_ato: __u32,
    pub tcpi_snd_mss: __u32,
    pub tcpi_rcv_mss: __u32,

    pub tcpi_unacked: __u32,
    pub tcpi_sacked: __u32,
    pub tcpi_lost: __u32,
    pub tcpi_retrans: __u32,
    pub tcpi_fackets: __u32,

    pub tcpi_last_data_sent: __u32,
    pub tcpi_last_ack_sent: __u32,
    pub tcpi_last_data_recv: __u32,
    pub tcpi_last_ack_recv: __u32,

    pub tcpi_pmtu: __u32,
    pub tcpi_rcv_ssthresh: __u32,
    pub tcpi_rtt: __u32,
    pub tcpi_rttvar: __u32,
    pub tcpi_snd_ssthresh: __u32,
    pub tcpi_snd_cwnd: __u32,
    pub tcpi_advmss: __u32,
    pub tcpi_reordering: __u32,

    pub tcpi_rcv_rtt: __u32,
    pub tcpi_rcv_space: __u32,

    pub tcpi_total_retrans: __u32,

    pub tcpi_pacing_rate: __u64,
    pub tcpi_max_pacing_rate: __u64,
    pub tcpi_bytes_acked: __u64,
    pub tcpi_bytes_received: __u64,
    pub tcpi_segs_out: __u32,
    pub tcpi_segs_in: __u32,

    pub tcpi_notsent_bytes: __u32,
    pub tcpi_min_rtt: __u32,
    pub tcpi_data_segs_in: __u32,
    pub tcpi_data_segs_out: __u32,

    pub tcpi_delivery_rate: __u64,

    pub tcpi_busy_time: __u64,
    pub tcpi_rwnd_limited: __u64,
    pub tcpi_sndbuf_limited: __u64,

    pub tcpi_delivered: __u32,
    pub tcpi_delivered_ce: __u32,

    pub tcpi_bytes_sent: __u64,
    pub tcpi_bytes_retrans: __u64,
    pub tcpi_dsack_dups: __u32,
    pub tcpi_reord_seen: __u32,

    pub tcpi_rcv_ooopack: __u32,

    pub tcpi_snd_wnd: __u32,
    pub tcpi_rcv_wnd: __u32,

    pub tcpi_rehash: __u32,

    pub tcpi_total_rto: __u16,
    pub tcpi_total_rto_recoveries: __u16,
    pub tcpi_total_rto_time: __u32,
}

#[cfg(target_os = "linux")]
pub fn get_tcp_info(stream: &TcpStream) -> Result<TcpInfo> {
    use std::os::fd::AsRawFd;

    let fd = stream.as_raw_fd();
    let mut tcp_info = TcpInfo::default();
    let mut tcp_info_len = std::mem::size_of::<TcpInfo>() as u32;

    let ret = unsafe {
        libc::getsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_INFO,
            &mut tcp_info as *mut _ as *mut libc::c_void,
            &mut tcp_info_len,
        )
    };
    if ret != 0 {
        return Err(Error::CallLibcFailed(std::io::Error::last_os_error()));
    }

    Ok(tcp_info)
}
