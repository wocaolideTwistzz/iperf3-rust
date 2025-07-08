use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    constant::{DEFAULT_BLOCK_SIZE, DEFAULT_INTERVAL_SEC},
    error::{ClientError, ServerError},
    opts::ClientOpts,
};

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
    SendResults(HashMap<usize, StreamStats>),
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
    SetState(State),
    SendResults(HashMap<usize, StreamStats>),
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
    pub prefer_ipv6: bool,

    // direction of the test
    pub direction: Direction,

    // duration of the test
    pub duration: u64,

    // number of parallel streams
    pub parallel: u16,

    pub client_version: String,

    // length of buffer to read or write (default 2 MB for TCP)
    pub block_size: usize,

    /// Set TCP no delay, disabling Nagle's Algorithm
    pub no_delay: bool,

    /// SO_SNDBUF/SO_RECVBUF for the data streams, uses the system default if unset.
    pub socket_buffers: Option<usize>,

    /// Set TCP/SCTP maximum segment size (MTU - 40 bytes)
    pub mss: Option<usize>,

    /// seconds between periodic throughput reports
    pub interval: u64,

    /// maximum length of data to send/recv
    pub max_length: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamStats {
    pub id: usize,
    pub index: Option<usize>,
    pub duration: u64, // ms
    pub bytes_transferred: usize,
    pub retransmits: Option<usize>, // tcp retransmits (Linux only)
    pub cwnd: Option<usize>,        // tcp cwnd (Linux only)
    pub is_peer: bool,
    pub is_summary: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum State {
    /// Parameters have been exchanged, but server is not ready yet to ask for data stream
    /// connections.
    Start,

    /// Asks the client to establish the data stream connections.
    CreateStreams {
        cookie: String,
    },

    /// All connections are established, stream the data and measure.
    Running,

    /// We are asked to exchange the TestResults between server and client. Client will initiate this
    /// exchange once it receives a transition into the state.
    ExchangeResults,

    Terminate,
}

impl Parameters {
    pub fn from_opts(opts: &ClientOpts) -> Self {
        let direction = if opts.bidir {
            Direction::Bidirectional
        } else if opts.reverse {
            Direction::ServerToClient
        } else {
            Direction::ClientToServer
        };

        Self {
            prefer_ipv6: opts.prefer_ipv6,
            direction,
            duration: opts.time,
            parallel: opts.parallel,
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            block_size: opts.length.unwrap_or(DEFAULT_BLOCK_SIZE),
            no_delay: opts.no_delay,
            socket_buffers: opts.window,
            mss: opts.set_mss,
            interval: opts.interval.unwrap_or(DEFAULT_INTERVAL_SEC),
            max_length: opts.bytes,
        }
    }
}
