use std::sync::Arc;

use tokio::{net::TcpStream, sync::{mpsc, Mutex}};

use crate::{
    error::Result,
    message::{Parameters, Role, ServerMessage, State, StreamStats},
    net_util::server_write_message,
};

/// A master object that holds the state of this perf test run
pub struct Perf {
    /// Bind local address with ipv6?
    pub prefer_ipv6: bool,

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
    pub stats_sender: Option<mpsc::Sender<StreamStats>>
}

impl Perf {
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
