use std::{
    net::SocketAddr,
    time::Duration,
};

use log::{debug, warn};
use tokio::{
    net::{TcpSocket, TcpStream},
    sync::{
        broadcast,
        mpsc::{self, Receiver, Sender},
    },
    task::JoinHandle,
    time::timeout,
};

use crate::{
    constant::INTERNAL_PORT_BUFFER,
    error::Result,
    message::{ClientMessage, Direction, Role, State, StreamStats},
    net_util::{TcpStreamExt, client_read_message, client_write_message},
    perf::Perf,
    stream_workers::{StreamWorker, WorkerMessage},
};

#[derive(Debug)]
pub enum ControllerMessage {
    /// Asks the controller to create a new stream using this socket.
    CreateStream(TcpStream),
    StreamTerminated(usize),
}

pub struct Controller {
    pub sender: Sender<ControllerMessage>,
    perf: Perf,
    stream_index: u16,
    receiver: Receiver<ControllerMessage>,
    publisher: broadcast::Sender<WorkerMessage>,
    workers: Vec<JoinHandle<Result<StreamStats>>>,
}

impl Controller {
    pub fn new(perf: Perf) -> Self {
        let (sender, receiver) = mpsc::channel(INTERNAL_PORT_BUFFER);
        let (publisher, _) = broadcast::channel(1);
        Controller {
            sender,
            stream_index: 0,
            perf,
            receiver,
            publisher,
            workers: vec![],
        }
    }

    /// This is the test controller code, when this function terminates, the test is done.
    pub async fn run_controller(mut self) -> Result<()> {
        debug!("Controller has started");
        let old_state = self.perf.state.clone().lock().await.clone();
        loop {
            let state = self.perf.state.clone().lock().await.clone();
            debug!("Controller state: {:?}", state);
            let role = &self.perf.role;

            match state {
                State::Start if matches!(role, Role::Server) => {
                    // We are a server, we just started the test.
                    debug!("Test is initializing");
                    self.perf
                        .set_state(State::CreateStreams {
                            cookie: self.perf.cookie.clone(),
                        })
                        .await?;
                    continue;
                }
                State::CreateStreams { cookie: _ } if matches!(role, Role::Server) => {
                    // We are a server, we are waiting for streams to be created.
                    debug!("Waiting for data streams to be connected");
                }
                State::CreateStreams { cookie: _ } if matches!(role, Role::Client) => {
                    // We are a client, we are being asked to create streams.
                    debug!("We should create data streams now");
                    self.create_streams().await?;
                }
                // Only process this on the transition. Start the load testing.
                State::Running if old_state != State::Running => {
                    debug!("Streams have been created, starting the load test");
                    _ = self.publisher.send(WorkerMessage::StartLoad)?;
                }
                // We are asked to exchange the test results we have.
                State::ExchangeResults => {
                    // Do we have active streams yet? We should ask these to terminate and wait
                    // This is best-effort.
                }
                _ => {}
            }
        }
        todo!()
    }

    /// Executed only once on the client. Established N connections to the server according to the
    /// exchanged `Parameters`
    async fn create_streams(&mut self) -> Result<()> {
        assert_eq!(self.perf.role, Role::Client);
        // Let's connect the send stream first. We will connect and authenticate streams
        // sequentially for simplicity. The server expects all the (client-to-server) streams
        // to be created first. That's an implicit assumption as part of the protocol.
        let total_needed_streams = self.perf.num_send_streams + self.perf.num_receive_streams;

        while self.stream_index < total_needed_streams - 1 {
            let stream = self.connect_data_stream().await?;
            self.create_and_register_stream_worker(stream);
        }
        Ok(())
    }

    /// Creates a connection to the server that serves as a data stream to test the network.
    /// The method initialize the connection and performs the authentication as well.
    async fn connect_data_stream(&self) -> Result<TcpStream> {
        let local_addr: SocketAddr = match self.perf.bind_address.as_ref() {
            Some(addr) => addr.parse()?,
            None => {
                if self.perf.prefer_ipv6 {
                    "[::]:0".parse()?
                } else {
                    "0.0.0.0:0".parse()?
                }
            }
        };
        let remote_addr: SocketAddr = self
            .perf
            .server_address
            .as_ref()
            .expect("empty server address")
            .parse()?;

        let socket = if local_addr.is_ipv4() {
            TcpSocket::new_v4()?
        } else {
            TcpSocket::new_v6()?
        };
        socket.bind(local_addr)?;

        debug!(
            "Opening a data stream: {} -> {}...",
            local_addr, remote_addr
        );
        let mut stream = socket.connect(remote_addr).await?;

        // Send the hello and authenticate。
        client_write_message(
            &mut stream,
            ClientMessage::Hello {
                cookie: self.perf.cookie.clone(),
            },
        )
        .await?;

        _ = client_read_message(&mut stream).await?;
        debug!(
            "Data stream created: {} -> {}",
            stream.local_addr_string(),
            stream.peer_addr_string()
        );
        Ok(stream)
    }

    fn create_and_register_stream_worker(&mut self, stream: TcpStream) {
        // The first (num_send_streams) are sending (meaning that we `Client` are sending
        // end of this stream)
        let mut is_sending = self.stream_index <= self.perf.num_send_streams;
        if self.perf.role == Role::Server && self.perf.params.direction == Direction::Bidirectional
        {
            is_sending = !is_sending;
        }
        let worker_id = self.stream_index as usize;
        let receiver = self.publisher.subscribe();
        let sender = self
            .perf
            .stats_sender
            .clone()
            .expect("stats sender should not empty");

        let worker = StreamWorker::new(
            worker_id,
            stream,
            self.perf.params.clone(),
            is_sending,
            sender,
            receiver,
        );
        let controller_sender = self.sender.clone();
        let handler = tokio::spawn(async move {
            let result = worker.run_worker().await?;
            controller_sender
                .try_send(ControllerMessage::StreamTerminated(worker_id))
                .unwrap_or_else(|e| debug!("Failed to communicate with controller: {e}"));
            Ok(result)
        });
        self.workers.push(handler);
        self.stream_index += 1;
    }

    async fn collect_stream_results(&mut self) -> Result<Vec<StreamStats>> {
        let mut stream_results = vec![];
        for (id, handler) in self.workers.drain(..).enumerate() {
            debug!("Waiting on stream {} to terminate", id);

            match timeout(Duration::from_secs(5), handler).await {
                Err(_) => warn!(
                    "Timeout waiting on stream {} to terminate, ignoring it.",
                    id
                ),
                Ok(Ok(Ok(result))) => stream_results.push(result),
                Ok(Ok(Err(e))) => warn!(
                    "Stream {} terminated with error ({}) ignoring its results!",
                    id, e
                ),
                Ok(Err(e)) => warn!("Failed to join stream {}: {}", id, e),
            }
        }
        Ok(stream_results)
    }
}
