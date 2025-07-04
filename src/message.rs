use serde::{Deserialize, Serialize};

use crate::error::{ClientError, ServerError};

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientEnvelope {
    Message(ClientMessage),
    Error(ClientError),
}

/// Message sent by the client to the server
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    /// The first message sent by the client to the server
    /// The cookie is a random uuid that the client uses to identify the session
    Hello { cookie: String },

    /// Send parameters to the server
    SendParameters(Parameters),

    /// Send results to the server
    SendResults(Vec<StreamStats>),
}

/// Message sent by the server to the client
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerEnvelope {
    Message(ServerMessage),
    Error(ServerError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Welcome message sent by the server to the client
    Welcome,

    /// Send results to the client
    SendResults(Vec<StreamStats>),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Role {
    Client,
    Server,
}

/// Direction of stream
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
    Bidirectional,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Parameters {
    // TODO: implement UDP
    // protocol:

    // direction of the test
    pub direction: Direction,

    // omit the first n seconds
    pub omit: u64,

    // duration of the test
    pub duration: u64,

    // number of parallel streams
    pub parallel: u16,

    pub client_version: String,

    // length of buffer to read or write (default 128 KB for TCP)
    pub block_size: usize,

    /// Set TCP no delay, disabling Nagle's Algorithm
    pub no_delay: bool,

    /// SO_SNDBUF/SO_RECVBUF for the data streams, uses the system default if unset.
    pub socket_buffers: Option<usize>,

    /// Set TCP/SCTP maximum segment size (MTU - 40 bytes)
    pub mss: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamStats {
    pub index: Option<usize>,
    pub duration: u64, // ms
    pub bytes_transferred: usize,
    pub retransmits: Option<usize>, // tcp retransmits (Linux only)
    pub cwnd: Option<usize>,        // tcp cwnd (Linux only)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum State {}
