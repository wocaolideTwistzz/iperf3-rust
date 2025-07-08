use clap::{ArgGroup, Parser};

#[derive(Debug, Parser, Clone)]
#[clap(groups = [
    ArgGroup::new("server_or_client").required(true),
    ArgGroup::new("direction")
])]
pub struct Opts {
    #[clap(flatten)]
    pub common: CommonOpts,

    #[clap(flatten)]
    pub server: ServerOpts,

    #[clap(flatten)]
    pub client: ClientOpts,
}

#[derive(Debug, Parser, Clone)]
pub struct CommonOpts {
    /// server port to listen on/connect to
    #[clap(short, long, default_value = "9201")]
    pub port: u16,

    /// emit debugging output
    #[clap(short, long)]
    pub debug: bool,

    /// bind to the interface associated with the address <host>
    #[clap(short = 'B', long)]
    pub bind: Option<String>,
}

#[derive(Debug, Parser, Clone)]
pub struct ServerOpts {
    /// run in server mode
    #[clap(short, long, group = "server_or_client")]
    pub server: bool,
}

#[derive(Debug, Parser, Clone)]
pub struct ClientOpts {
    /// run in client mode, connecting to <host>
    #[clap(short, long, group = "server_or_client")]
    pub client: Option<String>,

    /// time in seconds to transmit for (default 10 secs)
    #[clap(short, long, default_value = "10")]
    pub time: u64,

    /// number of bytes to transmit (will trigger the minimum value between time and bytes)
    #[clap(short = 'n', long)]
    pub bytes: Option<usize>,

    /// length of buffer to read or write
    /// (default 2 MB for TCP, dynamic or 1460 for UDP)
    #[clap(short, long)]
    pub length: Option<usize>,

    /// number of parallel client streams to run
    #[clap(short = 'P', long, default_value = "1")]
    pub parallel: u16,

    /// run in reverse mode (server sends, client receives)
    #[clap(short = 'R', long, group = "direction")]
    pub reverse: bool,

    /// run in bidirectional mode.
    /// Client and server send and receive data.
    #[clap(long, group = "direction")]
    pub bidir: bool,

    /// set window size / socket buffer size
    #[clap(short, long)]
    pub window: Option<usize>,

    /// set TCP/SCTP maximum segment size (MTU - 40 bytes)
    #[clap(short = 'M', long)]
    pub set_mss: Option<usize>,

    /// set TCP/SCTP no delay, disabling Nagle's Algorithm
    #[clap(short = 'N', long)]
    pub no_delay: bool,

    /// prefer IPv6
    #[clap(long)]
    pub prefer_ipv6: bool,

    /// seconds between periodic throughput reports
    #[clap(short = 'i', long)]
    pub interval: Option<u64>,
}
