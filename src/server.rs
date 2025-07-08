use crate::{
    controller::Controller,
    error::ServerError,
    net_util::{TcpStreamExt, server_write_error},
    ui,
};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Arc, Weak},
};

use log::{debug, error, info, warn};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, broadcast, mpsc},
    time::timeout,
};

use crate::{
    constant::PROTOCOL_TIMEOUT,
    controller::ControllerMessage,
    error::{Error, Result},
    message::{ClientMessage, Role, ServerMessage, State, StreamStats},
    net_util::{server_read_message, server_write_message},
    opts::CommonOpts,
    perf::Perf,
};

pub async fn start_server(
    common_opts: &CommonOpts,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<mpsc::Receiver<PerfStats>> {
    let listener = if let Some(bind) = common_opts.bind.as_ref() {
        TcpListener::bind(format!("{}:{}", bind, common_opts.port)).await?
    } else {
        TcpListener::bind(
            &[
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), common_opts.port),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), common_opts.port),
            ][..],
        )
        .await?
    };

    let (tx, rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut in_flight_test_state: Weak<Mutex<State>> = Weak::new();
        let mut controller_channel: Option<mpsc::Sender<ControllerMessage>> = None;

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Server terminate!");
                    return;
                }
                inbound = listener.accept() => match inbound {
                    Ok((inbound, _)) => {
                        // Do we have a test in-flight?
                        match in_flight_test_state.upgrade() {
                            Some(state_lock) => {
                                if let Err(e) = handle_create_stream(inbound, state_lock, controller_channel.clone().unwrap()).await {
                                    // Unexpect error, terminate this stream.
                                    warn!("Handle create stream error: {e:?}");
                                    controller_channel.clone().unwrap()
                                        .try_send(ControllerMessage::Terminate)
                                        .unwrap_or_else(|e| warn!("Server try send control message error: {e:?}"));
                                }
                            }
                            None => {
                                let peer_string = inbound.peer_addr_string();

                                // No in-flight test, let's create one.
                                info!("Accepted connection from {peer_string}");

                                // Do we have an active test running already?
                                // If not, let's start a test session and wait for parameters from the client.
                                match create_perf(inbound, shutdown.resubscribe()).await {
                                    Ok((perf, perf_stats)) => {
                                        info!("[{peer_string}] Test Created");

                                        // Send perf_stats
                                        _ = tx.send(PerfStats {
                                            peer_addr: peer_string.clone(),
                                            receiver: perf_stats,
                                         }).await;

                                        // Keep a weak-ref to this test here.
                                        in_flight_test_state = Arc::downgrade(&perf.state);

                                        let controller = Controller::new(perf);
                                        controller_channel = Some(controller.sender.clone());

                                        // Async dispatch.
                                        tokio::spawn(async move {
                                            if let Err(e) = controller.run_controller().await {
                                                error!("Controller aborted: {e:?}");
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        error!("[{peer_string}] {e:?}");
                                    }
                                }
                            }
                        }

                    }
                    Err(e) => {
                        error!("Listener accept error: {e:?}");
                        return;
                    }
                }
            }
        }
    });

    Ok(rx)
}

pub async fn run_server(common_opts: &CommonOpts, shutdown: broadcast::Receiver<()>) {
    ui::print_server_banner(common_opts);

    let mut perf_stats_receiver = start_server(common_opts, shutdown).await.unwrap();

    while let Some(mut perf_stats) = perf_stats_receiver.recv().await {
        ui::print_accept(perf_stats.peer_addr);
        while let Some(stat) = perf_stats.receiver.recv().await {
            ui::print_stats(&stat);
        }
    }
}

async fn create_perf(
    mut control_socket: TcpStream,
    shutdown: broadcast::Receiver<()>,
) -> Result<(Perf, mpsc::Receiver<StreamStats>)> {
    let cookie = read_cookie(&mut control_socket).await?;
    debug!("Hello received: {cookie}");

    server_write_message(&mut control_socket, ServerMessage::Welcome).await?;

    let params = match timeout(PROTOCOL_TIMEOUT, server_read_message(&mut control_socket)).await?? {
        ClientMessage::SendParameters(params) => params,
        e => return Err(Error::UnexpectedClientMessage(e)),
    };

    Ok(Perf::new_with_stats_receiver(
        None,
        None,
        cookie,
        Role::Server,
        params,
        control_socket,
        shutdown,
    ))
}

async fn read_cookie(control_socket: &mut TcpStream) -> Result<String> {
    match server_read_message(control_socket).await {
        Ok(ClientMessage::Hello { cookie }) => Ok(cookie),
        Ok(e) => Err(Error::UnexpectedClientMessage(e)),
        Err(e) => Err(e),
    }
}

async fn handle_create_stream(
    mut inbound: TcpStream,
    current_state: Arc<Mutex<State>>,
    controller_sender: mpsc::Sender<ControllerMessage>,
) -> Result<()> {
    let peer_string = inbound.peer_addr_string();

    if let State::CreateStreams { ref cookie } = *current_state.lock().await {
        // Validate the cookie in this case.
        // Read the cookie from the Hello message and compare to cookie.

        let client_cookie = read_cookie(&mut inbound).await?;

        if client_cookie == *cookie {
            // Create the stream.
            server_write_message(&mut inbound, ServerMessage::Welcome).await?;
            controller_sender
                .try_send(ControllerMessage::CreateStream(inbound))
                .unwrap_or_else(|e| warn!("Server try send control message error: {e:?}"));
        } else {
            // We already have a test in-flight, close the connection immediately.
            // Note that here we don't read anything from the socket to avoid
            // unnecessarily being blocked on the client not sending any data.
            info!("Test already in-flight, rejecting connection from {peer_string}");
            let _ = server_write_error(&mut inbound, ServerError::PerfTestAlreadyInFlight).await;
        }
    } else {
        // We already have a test in-flight, close the connection immediately.
        // Note that here we don't read anything from the socket to avoid
        // unnecessarily being blocked on the client not sending any data.
        info!("Test already in-flight, rejecting connection from {peer_string}");
        let _ = server_write_error(&mut inbound, ServerError::PerfTestAlreadyInFlight).await;
    }
    Ok(())
}

pub struct PerfStats {
    pub peer_addr: String,
    pub receiver: mpsc::Receiver<StreamStats>,
}
