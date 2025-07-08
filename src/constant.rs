use std::time::Duration;

// Bytes
pub const KB: usize = 1024;
pub const MB: usize = 1024 * KB;
pub const GB: usize = 1024 * MB;
pub const TB: usize = 1024 * GB;

// Bitrate
pub const K_BITS_PER_SEC: usize = 1000;
pub const M_BITS_PER_SEC: usize = 1000 * K_BITS_PER_SEC;
pub const G_BITS_PER_SEC: usize = 1000 * M_BITS_PER_SEC;
pub const T_BITS_PER_SEC: usize = 1000 * G_BITS_PER_SEC;

pub const INTERNAL_PORT_BUFFER: usize = 5;
pub const MESSAGE_LENGTH_SIZE_BYTES: usize = std::mem::size_of::<u32>();
pub const MAX_CONTROL_MESSAGE_SIZE: u32 = 20 * MB as u32;
pub const DEFAULT_BLOCK_SIZE: usize = 2 * MB;
pub const DEFAULT_INTERVAL_SEC: u64 = 1;
pub const PROTOCOL_TIMEOUT: Duration = Duration::from_secs(10);
