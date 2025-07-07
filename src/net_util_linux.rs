use tokio::net::{TcpSocket, TcpStream};

use crate::error::{Error, Result};
use libc::{__u8, __u16, __u32, __u64};
use std::os::fd::AsRawFd;

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

pub fn get_tcp_info(stream: &TcpStream) -> Result<TcpInfo> {
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
        return Err(Error::CallLibcError(std::io::Error::last_os_error()));
    }

    Ok(tcp_info)
}

pub fn set_mss(socket: &TcpSocket, mss: i32) -> Result<()> {
    let fd = socket.as_raw_fd();
    let ret = unsafe {
        libc::setsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_MAXSEG,
            &mss as *const _ as *const libc::c_void,
            std::mem::size_of_val(&mss) as libc::socklen_t,
        )
    };
    if ret != 0 {
        return Err(Error::CallLibcError(std::io::Error::last_os_error()));
    }
    Ok(())
}
