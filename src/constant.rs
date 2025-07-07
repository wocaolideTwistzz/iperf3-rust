use std::time::Duration;

pub const INTERNAL_PORT_BUFFER: usize = 5;
pub const MB: usize = 1024 * 1024;
pub const MESSAGE_LENGTH_SIZE_BYTES: usize = std::mem::size_of::<u32>();
pub const MAX_CONTROL_MESSAGE_SIZE: u32 = 20 * MB as u32;
pub const DEFAULT_BLOCK_SIZE: usize = 2 * MB;
pub const PROTOCOL_TIMEOUT: Duration = Duration::from_secs(10);
