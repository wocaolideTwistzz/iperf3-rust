use clap::Parser;
use iperf3_rust::{client::run_client, opts::Opts};

#[tokio::main]
async fn main() {
    let opts = Opts::parse();

    let control_c = tokio::signal::ctrl_c();
    if opts.server.server {
    } else {
        run_client(&opts.common, &opts.client, control_c).await;
    }
}
