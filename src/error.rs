use serde::{Deserialize, Serialize};

use crate::{
    constant::MAX_CONTROL_MESSAGE_SIZE,
    message::{ClientMessage, ServerMessage, StreamStats},
    stream_workers::WorkerMessage,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Worker terminated")]
    WorkerTerminated,

    #[error("Parse address error: {0}")]
    ParseAddressError(#[from] std::net::AddrParseError),

    #[error("Call libc error: {0}")]
    CallLibcError(#[source] std::io::Error),

    #[error("Stream worker error: {0}")]
    StreamWorkerError(#[source] StreamWorkerError),

    #[error("Publish worker message error: {0}")]
    PublishWorkerMessageError(#[from] tokio::sync::broadcast::error::SendError<WorkerMessage>),

    #[error("Unexpected server message: {0:?}")]
    UnexpectedServerMessage(ServerMessage),

    #[error("Unexpected client message: {0:?}")]
    UnexpectedClientMessage(ClientMessage),

    #[error("Send stream stats error: {0}")]
    SendStreamStatsError(#[from] tokio::sync::mpsc::error::SendError<StreamStats>),

    #[error("Join task error: {0}")]
    JoinTaskError(#[from] tokio::task::JoinError),

    #[error("Server write error: {0}")]
    ServerWriteError(#[source] NetUtilError),

    #[error("Protocol read timeout")]
    ProtocolReadTimeout(#[from] tokio::time::error::Elapsed),

    #[error("Server read error: {0}")]
    ServerReadError(#[source] NetUtilError),

    #[error("Client write error: {0}")]
    ClientWriteError(#[source] NetUtilError),

    #[error("Client read error: {0}")]
    ClientReadError(#[source] NetUtilError),

    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error(transparent)]
    ServerError(#[from] ServerError),
}

#[derive(Debug, thiserror::Error)]
pub enum NetUtilError {
    #[error(transparent)]
    SerializeError(#[from] serde_json::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Control message too large: {0}, max allowed: {MAX_CONTROL_MESSAGE_SIZE}")]
    ControlMessageTooLarge(u32),
 
    #[error("Connected was closed by peer")]
    ConnectionWasClosedByPeer,
}

#[derive(Debug, thiserror::Error)]
pub enum StreamWorkerError {
    #[error(transparent)]
    ReceiveError(#[from] tokio::sync::broadcast::error::RecvError),

    #[error(transparent)]
    StreamWorkerSendError(#[from] tokio::sync::mpsc::error::SendError<StreamStats>),

    #[error(transparent)]
    StreamWorkerTimeout(#[from] tokio::time::error::Elapsed),

    #[error(transparent)]
    StreamWorkerIoError(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum ClientError {}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum ServerError {
    #[error("perf test already in-flight")]
    PerfTestAlreadyInFlight,
}

pub type Result<T> = std::result::Result<T, Error>;
