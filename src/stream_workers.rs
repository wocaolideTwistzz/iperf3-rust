#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, FromRawSocket};
use std::time::{Duration, Instant};

use futures::FutureExt;
use log::{debug, warn};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{broadcast::Receiver, mpsc::Sender},
    time::timeout,
};

use crate::{
    error::{Error, Result},
    message::{Parameters, StreamStats},
};

#[derive(Debug, Clone)]
pub enum WorkerMessage {
    StartLoad,
    Terminate,
}

pub struct StreamWorker {
    pub id: usize,
    pub stream: TcpStream,
    pub is_sending: bool,
    pub params: Parameters,
    sender: Sender<StreamStats>,
    receiver: Receiver<WorkerMessage>,
}

impl StreamWorker {
    pub fn new(
        id: usize,
        stream: TcpStream,
        params: Parameters,
        is_sending: bool,
        sender: Sender<StreamStats>,
        receiver: Receiver<WorkerMessage>,
    ) -> Self {
        Self {
            id,
            stream,
            is_sending,
            params,
            sender,
            receiver,
        }
    }

    pub async fn run_worker(mut self) -> Result<StreamStats> {
        // Let's pre-allocate a buffer for 1 block.
        let mut buffer = vec![0_u8; self.params.block_size];

        self.configure_stream_socket()?;

        debug!(
            "Data stream {} crated ({}), waiting for the StartLoad signal!",
            self.id,
            if self.is_sending {
                "sending"
            } else {
                "receiving"
            }
        );

        let signal = self
            .receiver
            .recv()
            .await
            .map_err(|e| Error::StreamWorkerError(e.into()))?;
        if !matches!(signal, WorkerMessage::StartLoad) {
            return Err(Error::WorkerTerminated);
        }

        let interval = Duration::from_secs(self.params.interval);
        let start_time = Instant::now();
        let timeout_duration = Duration::from_secs(self.params.duration);

        let mut index = 0;

        let mut bytes_transferred = 0_usize;
        let mut total_bytes_transferred = 0_usize;
        let mut total_retransmits = 0_usize;
        let mut total_cwnd = 0_usize;

        let mut current_interval_start = Instant::now();

        loop {
            if start_time.elapsed() > timeout_duration {
                debug!("Test time is up!");
                break;
            }
            if self
                .params
                .max_length
                .is_some_and(|v| v > total_bytes_transferred)
            {
                debug!("Test bytes is up!");
                break;
            }

            match self
                .receiver
                .recv()
                .now_or_never()
                .transpose()
                .map_err(|e| Error::StreamWorkerError(e.into()))?
            {
                Some(WorkerMessage::Terminate) => break,
                Some(WorkerMessage::StartLoad) => {
                    warn!("Unexpected StartLoad signal received!")
                }
                None => {}
            }

            let read_or_write = if self.is_sending {
                self.stream.write(&buffer).left_future()
            } else {
                self.stream.read(&mut buffer).right_future()
            };

            if let Ok(bytes_count) = timeout(Duration::from_millis(100), read_or_write).await {
                let bytes_count = bytes_count.map_err(|e| Error::StreamWorkerError(e.into()))?;
                if bytes_count > 0 {
                    bytes_transferred += bytes_count;
                } else {
                    warn!("Stream {}'s connection has been closed", self.id);
                    break;
                }
            } else {
                debug!(
                    "Stream {} taking logger than 100 ms to produce data",
                    self.id
                );
            }

            let now = Instant::now();
            let current_duration = now.duration_since(current_interval_start);
            if current_duration >= interval {
                let stats = {
                    #[cfg(target_os = "linux")]
                    {
                        let tcp_info = crate::net_util_linux::get_tcp_info(&self.stream)
                            .unwrap_or_else(|e| {
                                use crate::net_util_linux::TcpInfo;

                                warn!("Failed to get TCP info: {e:?}");
                                TcpInfo::default()
                            });

                        total_retransmits += tcp_info.tcpi_retransmits as usize;
                        total_cwnd += tcp_info.tcpi_snd_cwnd as usize;

                        StreamStats {
                            id: self.id,
                            index: Some(index),
                            duration: current_duration.as_millis() as u64,
                            bytes_transferred,
                            retransmits: Some(tcp_info.tcpi_retransmits as usize),
                            cwnd: Some(
                                tcp_info.tcpi_snd_cwnd as usize * tcp_info.tcpi_snd_mss as usize,
                            ),
                            is_peer: false,
                            is_summary: false,
                        }
                    }

                    #[cfg(not(target_os = "linux"))]
                    StreamStats {
                        id: self.id,
                        index: Some(index),
                        duration: current_duration.as_millis() as u64,
                        bytes_transferred,
                        retransmits: None,
                        cwnd: None,
                        is_peer: false,
                        is_summary: false,
                    }
                };

                self.sender
                    .send(stats)
                    .await
                    .map_err(|e| Error::StreamWorkerError(e.into()))?;
                total_bytes_transferred += bytes_transferred;
                current_interval_start = now;
                bytes_transferred = 0;
                index += 1;
            }
        }

        // Drain the sockets if we are receiving end, we need to do that to avoid failing the
        // sender stream that might still be sending data.
        if !self.is_sending {
            let mut stream = self.stream;

            tokio::spawn(
                async move { while stream.read(&mut buffer).await.is_ok_and(|v| v > 0) {} },
            );
        }

        Ok(StreamStats {
            id: self.id,
            index: None,
            duration: start_time.elapsed().as_millis() as u64,
            bytes_transferred: total_bytes_transferred,
            retransmits: Some(total_retransmits),
            cwnd: Some(total_cwnd),
            is_peer: false,
            is_summary: true,
        })
    }

    fn configure_stream_socket(&mut self) -> Result<()> {
        if self.params.no_delay {
            self.stream
                .set_nodelay(self.params.no_delay)
                .unwrap_or_else(|e| warn!("Failed to set no delay: {e:?}"));
        }

        if let Some(socket_buffers) = self.params.socket_buffers {
            let socket_buffers = socket_buffers.try_into().unwrap_or(u32::MAX);
            debug!("Setting socket buffer size to {socket_buffers}");

            unsafe {
                #[cfg(unix)]
                let sock = tokio::net::TcpSocket::from_raw_fd(self.stream.as_raw_fd());

                #[cfg(windows)]
                let sock = tokio::net::TcpSocket::from_raw_socket(self.stream.as_raw_socket());

                sock.set_recv_buffer_size(socket_buffers)
                    .unwrap_or_else(|e| warn!("Failed to set recv socket buffer size: {e:?}"));

                sock.set_send_buffer_size(socket_buffers)
                    .unwrap_or_else(|e| warn!("Failed to set send socket buffer size: {e:?}"));

                std::mem::forget(sock);
            };
        }
        Ok(())
    }
}
