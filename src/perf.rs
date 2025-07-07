use std::sync::Arc;

use tokio::{
    net::TcpStream,
    sync::{Mutex, broadcast, mpsc},
};

use crate::{
    constant::INTERNAL_PORT_BUFFER,
    error::Result,
    message::{Direction, Parameters, Role, ServerMessage, State, StreamStats},
    net_util::server_write_message,
};

/// A master object that holds the state of this perf test run
pub struct Perf {
    /// Bind local address.
    pub bind_address: Option<String>,

    /// Server address that client should connect to.
    pub server_address: Option<String>,

    /// A unique identifier for this test run, used by clients to authenticate data streams.
    pub cookie: String,

    /// Represents where we are in the life-cycle of a test
    pub state: Arc<Mutex<State>>,

    /// The control socket stream
    pub control_socket: TcpStream,

    /// Defines whether we are a server or client in the test run.
    pub role: Role,

    /// The test configuration.
    pub params: Parameters,

    /// The number of streams at which we are sending data.
    pub num_send_streams: u16,

    /// The number of streams at which we are receiving data.
    pub num_receive_streams: u16,

    /// Stream stats sender.
    pub(crate) stats_sender: Option<mpsc::Sender<StreamStats>>,

    /// A broadcast receiver that is used to signal shutdown.   
    pub shutdown: broadcast::Receiver<()>,
}

impl Perf {
    pub fn new_with_stats_receiver(
        bind_address: Option<String>,
        server_address: Option<String>,
        cookie: String,
        role: Role,
        params: Parameters,
        control_socket: TcpStream,
        shutdown: broadcast::Receiver<()>,
    ) -> (Self, mpsc::Receiver<StreamStats>) {
        let mut num_send_streams = 0;
        let mut num_receive_streams = 0;
        match params.direction {
            Direction::ClientToServer => num_send_streams = params.parallel,
            Direction::ServerToClient => num_receive_streams = params.parallel,
            Direction::Bidirectional => {
                num_send_streams = params.parallel;
                num_receive_streams = params.parallel;
            }
        }
        if matches!(role, Role::Server) {
            std::mem::swap(&mut num_send_streams, &mut num_receive_streams);
        }
        let (tx, rx) = mpsc::channel(INTERNAL_PORT_BUFFER);
        let perf = Perf {
            bind_address,
            server_address,
            cookie,
            state: Arc::new(Mutex::new(State::Start)),
            control_socket,
            role,
            params,
            num_send_streams,
            num_receive_streams,
            stats_sender: Some(tx),
            shutdown,
        };
        (perf, rx)
    }

    pub async fn set_state(&mut self, state: State) -> Result<()> {
        if self.role == Role::Server {
            server_write_message(
                &mut self.control_socket,
                ServerMessage::SetState(state.clone()),
            )
            .await?;
        }

        let mut locked_state = self.state.lock().await;
        *locked_state = state;
        Ok(())
    }
}
