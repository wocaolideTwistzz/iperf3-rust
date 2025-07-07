use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use log::{debug, info};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc},
    time::timeout,
};

use crate::{
    constant::PROTOCOL_TIMEOUT,
    error::{Error, Result},
    message::{ClientMessage, Role, ServerMessage, StreamStats},
    net_util::{server_read_message, server_write_message},
    opts::CommonOpts,
    perf::Perf,
};

pub async fn start_server(
    common_opts: &CommonOpts,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<mpsc::Receiver<StreamStats>> {
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

    let (stats_tx, stats_rx) = mpsc::channel(100);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    info!("Server shutdown signal received");
                    break;
                }
                inbound = listener.accept() => {
                    match inbound {
                        Ok((stream, addr)) => {
                            debug!("Accepted connection from: {}", addr);
                            let stats_tx = stats_tx.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_client(stream, stats_tx).await {
                                    error!("Error handling client: {}", e);
                                    return; // Fix the syntax error
                                }
                            });
                        }
                        Err(e) => {
                            error!("Error accepting connection: {}", e);
                            continue;
                        }
                    }
                }
            }
        }
        // Drop the sender to indicate we're done
        drop(stats_tx);
    });

    Ok(stats_rx)
}

async fn create_perf(mut control_socket: TcpStream) -> Result<(Perf, mpsc::Receiver<StreamStats>)> {
    let cookie = read_cookie(&mut control_socket).await?;
    debug!("Hello received: {}", cookie);

    server_write_message(&mut control_socket, ServerMessage::Welcome).await?;

    match timeout(PROTOCOL_TIMEOUT, server_read_message(&mut control_socket)).await?? {
        ClientMessage::SendParameters(params) => {}
        e => return Err(Error::UnexpectedClientMessage(e)),
    }

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
