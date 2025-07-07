use std::{collections::HashMap, net::SocketAddr, time::Duration};

use log::{debug, info, warn};
use tokio::{
    net::TcpStream,
    sync::{
        broadcast,
        mpsc::{self, Receiver, Sender},
    },
    task::JoinHandle,
    time::timeout,
};

use crate::{
    constant::INTERNAL_PORT_BUFFER,
    error::{Error, Result},
    message::{ClientMessage, Direction, Role, ServerMessage, State, StreamStats},
    net_util::{
        TcpStreamExt, client_read_message, client_write_message, new_socket, server_read_message,
        server_write_message,
    },
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
    workers: HashMap<usize, JoinHandle<Result<StreamStats>>>,
    stream_results: HashMap<usize, StreamStats>,
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
            workers: HashMap::new(),
            stream_results: HashMap::new(),
        }
    }

    /// This is the test controller code, when this function terminates, the test is done.
    pub async fn run_controller(mut self) -> Result<()> {
        debug!("Controller has started");
        let mut old_state = self.perf.state.clone().lock().await.clone();
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
                    debug!("Exchanging test results");
                    _ = self
                        .publisher
                        .send(WorkerMessage::Terminate)
                        .unwrap_or_else(|e| {
                            debug!("Failed to send terminate signal: {e}");
                            0
                        });
                    let local_results = self.collect_stream_results().await?;
                    let remote_results = self.exchange_results().await?;

                    if let Some(sender) = self.perf.stats_sender.take() {
                        for (_, stats) in local_results {
                            sender.send(stats).await?;
                        }
                        for (_, stats) in remote_results {
                            sender.send(stats).await?;
                        }
                    }
                    break;
                }
                // We received a terminate signal, we should ask the workers to terminate and wait
                State::Terminate => {
                    debug!("Receive terminate signal");
                    _ = self
                        .publisher
                        .send(WorkerMessage::Terminate)
                        .unwrap_or_else(|e| {
                            debug!("Failed to send terminate signal: {e}");
                            0
                        });
                    let local_results = self.collect_stream_results().await?;
                    if let Some(sender) = self.perf.stats_sender.take() {
                        for (_, stats) in local_results {
                            sender.send(stats).await?;
                        }
                    }
                    return Ok(());
                }
                _ => {}
            }
            old_state = state;

            if self.perf.shutdown.try_recv().is_ok() {
                self.perf.set_state(State::Terminate).await?;
                continue;
            }

            if self.perf.role == Role::Server {
                // Server
                let message = self.receiver.recv().await;
                self.process_internal_message(message).await?;
            } else {
                // Client
                tokio::select! {
                    internal_message = self.receiver.recv() => {
                        self.process_internal_message(internal_message).await?;
                    },
                    server_message = client_read_message(&mut self.perf.control_socket) => {
                        self.process_server_message(server_message?).await?;
                    }
                    else => break,
                }
            }
        }
        info!("iperf Done");
        Ok(())
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
        let socket = new_socket(self.perf.bind_address.clone(), self.perf.params.prefer_ipv6)?;

        let remote_addr: SocketAddr = self
            .perf
            .server_address
            .as_ref()
            .expect("empty server address")
            .parse()?;

        #[cfg(target_os = "linux")]
        if let Some(mss) = self.perf.params.mss {
            crate::net_util_linux::set_mss(&socket, mss as i32)
                .unwrap_or_else(|e| warn!("Failed to set MSS: {}", e));
        }

        debug!(
            "Opening a data stream: {:?} -> {}...",
            socket.local_addr(),
            remote_addr
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
        self.workers.insert(worker_id, handler);
        self.stream_index += 1;
    }

    async fn collect_stream_results(&mut self) -> Result<HashMap<usize, StreamStats>> {
        for (id, handler) in self.workers.drain() {
            debug!("Waiting on stream {} to terminate", id);

            match timeout(Duration::from_secs(5), handler).await {
                Err(_) => warn!(
                    "Timeout waiting on stream {} to terminate, ignoring it.",
                    id
                ),
                Ok(Ok(Ok(result))) => _ = self.stream_results.insert(id, result),
                Ok(Ok(Err(e))) => warn!(
                    "Stream {} terminated with error ({}) ignoring its results!",
                    id, e
                ),
                Ok(Err(e)) => warn!("Failed to join stream {}: {}", id, e),
            }
        }
        Ok(self.stream_results.clone())
    }

    /// Exchanges the test results.
    async fn exchange_results(&mut self) -> Result<HashMap<usize, StreamStats>> {
        if self.perf.role == Role::Client {
            // Send ours then read the peer's results.
            client_write_message(
                &mut self.perf.control_socket,
                ClientMessage::SendResults(self.stream_results.clone()),
            )
            .await?;

            match client_read_message(&mut self.perf.control_socket).await? {
                ServerMessage::SendResults(mut results) => {
                    results.iter_mut().for_each(|(_, v)| v.is_peer = true);
                    Ok(results)
                }
                m => Err(Error::UnexpectedServerMessage(m)),
            }
        } else {
            let remote_result = match server_read_message(&mut self.perf.control_socket).await? {
                ClientMessage::SendResults(mut results) => {
                    results.iter_mut().for_each(|(_, v)| v.is_peer = true);
                    results
                }
                m => return Err(Error::UnexpectedClientMessage(m)),
            };
            server_write_message(
                &mut self.perf.control_socket,
                ServerMessage::SendResults(self.stream_results.clone()),
            )
            .await?;
            Ok(remote_result)
        }
    }

    /// A handler when we receive a ControllerMessage from other components
    async fn process_internal_message(&mut self, message: Option<ControllerMessage>) -> Result<()> {
        match message {
            Some(ControllerMessage::CreateStream(stream)) => self.accept_stream(stream).await?,
            Some(ControllerMessage::StreamTerminated(id)) => {
                if let Some(handler) = self.workers.remove(&id) {
                    match handler.await? {
                        Ok(result) => _ = self.stream_results.insert(id, result),
                        Err(e) => warn!(
                            "Stream {id} has terminated with an error, We cannot fetch its total stream stats: {e:?}"
                        ),
                    }
                }
                if self.workers.is_empty() && self.perf.role == Role::Server {
                    self.perf.set_state(State::ExchangeResults).await?;
                }
            }
            None => {
                warn!("receive empty control message")
            }
        }
        Ok(())
    }

    async fn process_server_message(&mut self, message: ServerMessage) -> Result<()> {
        match message {
            ServerMessage::SetState(state) => self.perf.set_state(state).await?,
            e => warn!("Unexpected server message: {e:?}"),
        }
        Ok(())
    }

    /// Executed only on the server
    async fn accept_stream(&mut self, stream: TcpStream) -> Result<()> {
        assert_eq!(self.perf.role, Role::Server);

        let total_needed_stream =
            (self.perf.num_send_streams + self.perf.num_receive_streams) as usize;
        self.create_and_register_stream_worker(stream);
        if self.workers.len() == total_needed_stream {
            self.perf.set_state(State::Running).await?;
        }
        Ok(())
    }
}
