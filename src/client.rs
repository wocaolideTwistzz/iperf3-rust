use log::{debug, error, info};
use tokio::sync::{broadcast, mpsc};

use crate::{
    controller::Controller,
    error::Result,
    message::{ClientMessage, Parameters, Role, StreamStats},
    net_util::{TcpStreamExt, client_read_message, client_write_message, new_socket},
    opts::{ClientOpts, CommonOpts},
    perf::Perf,
    ui,
};

pub async fn start_client(
    common_opts: &CommonOpts,
    client_opts: &ClientOpts,
    shutdown: broadcast::Receiver<()>,
) -> Result<mpsc::Receiver<StreamStats>> {
    let server_addr = format!(
        "{}:{}",
        client_opts.client.as_ref().unwrap(),
        common_opts.port
    );

    info!("Connecting to {server_addr}...");
    let socket = new_socket(common_opts.bind.clone(), client_opts.prefer_ipv6)?;

    let mut control_socket = socket.connect(server_addr.parse()?).await?;
    info!(
        "Connected {} -> {}.",
        control_socket.local_addr_string(),
        control_socket.peer_addr_string()
    );

    let cookie = uuid::Uuid::new_v4().hyphenated().to_string();

    client_write_message(
        &mut control_socket,
        ClientMessage::Hello {
            cookie: cookie.clone(),
        },
    )
    .await?;
    debug!("Hello sent!");

    let _ = client_read_message(&mut control_socket).await?;
    debug!("Welcome received!");

    let params = Parameters::from_opts(client_opts);
    client_write_message(
        &mut control_socket,
        ClientMessage::SendParameters(params.clone()),
    )
    .await?;
    debug!("Parameters sent!");

    let (perf, rx) = Perf::new_with_stats_receiver(
        common_opts.bind.clone(),
        Some(server_addr),
        cookie,
        Role::Client,
        params,
        control_socket,
        shutdown,
    );

    let controller = Controller::new(perf);
    tokio::spawn(async move {
        if let Err(e) = controller.run_controller().await {
            error!("Controller error: {e:?}")
        }
    });
    Ok(rx)
}

pub async fn run_client(
    common_opts: &CommonOpts,
    client_opts: &ClientOpts,
    shutdown: broadcast::Receiver<()>,
) {
    ui::print_client_banner(common_opts, client_opts);
    let mut rx = start_client(common_opts, client_opts, shutdown)
        .await
        .unwrap();

    ui::print_header();
    while let Some(stats) = rx.recv().await {
        ui::print_stats(&stats);
    }
}
