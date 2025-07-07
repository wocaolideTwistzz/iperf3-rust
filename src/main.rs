use clap::Parser;
use iperf3_rust::{client::run_client, opts::Opts, server::run_server};
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    let opts = Opts::parse();

    let shutdown = new_shutdown();
    env_logger::builder().init();

    if opts.server.server {
        run_server(&opts.common, shutdown).await;
    } else {
        run_client(&opts.common, &opts.client, shutdown).await;
    }
}

fn new_shutdown() -> broadcast::Receiver<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    let (tx, rx) = broadcast::channel(1);

    tokio::spawn(async move {
        _ = ctrl_c.await;
        _ = tx.send(())
    });

    rx
}
