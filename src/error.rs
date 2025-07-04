use serde::{Deserialize, Serialize};

use crate::{constant::MAX_CONTROL_MESSAGE_SIZE, message::StreamStats};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Serialize error: {0}")]
    SerializeError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Control message too large: {0}, max allowed: {MAX_CONTROL_MESSAGE_SIZE}")]
    ControlMessageTooLarge(u32),

    #[error("Worker terminated")]
    WorkerTerminated,

    #[error("Worker stream receive error: {0}")]
    WorkerStreamReceiveError(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Worker stream send error: {0}")]
    WorkerStreamSendError(#[from] tokio::sync::mpsc::error::SendError<StreamStats>),

    #[error("Call libc failed: {0}")]
    CallLibcFailed(#[source] std::io::Error),

    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error(transparent)]
    ServerError(#[from] ServerError),
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum ClientError {}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum ServerError {}

pub type Result<T> = std::result::Result<T, Error>;
