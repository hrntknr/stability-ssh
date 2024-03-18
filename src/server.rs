use crate::{queue, utils};
use anyhow::Result;
use clap::Parser;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Parser, Debug, Clone)]
#[clap(name = "server")]
pub struct Opt {
    #[clap(long = "listen", short = 'l', default_value = "0.0.0.0:2222")]
    listen: SocketAddr,

    #[clap(long = "forward", short = 'f', default_value = "localhost:22")]
    forward: String,

    #[clap(long = "bufsize", short = 'b')]
    bufsize: Option<u32>,
}

pub async fn run(opt: Opt) -> Result<()> {
    let (cert_der, priv_key) = utils::gen_cert()?;
    let mut server_crypto = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(utils::SkipClientVerification::new())
        .with_single_cert(
            vec![rustls::Certificate(cert_der.clone())],
            rustls::PrivateKey(priv_key),
        )?;
    server_crypto.alpn_protocols = vec![b"stablessh".to_vec()];

    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server_crypto));
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());
    transport_config.max_idle_timeout(Some(Duration::from_secs(10).try_into()?));

    let endpoint = quinn::Endpoint::server(server_config, opt.listen)?;
    let endpoint_clone = endpoint.clone();
    let signal_thread = utils::stop_signal_wait();

    tokio::select! {
        _ = async move{
            while let Some(conn) = endpoint.accept().await {
                let fut = handle_connection(opt.clone(), conn);
                tokio::spawn(async move {
                    if let Err(e) = fut.await {
                        log::error!("{:?}", e);
                    }
                });
            }
        } => {}
        _ = signal_thread => {}
    }
    endpoint_clone.close(0_u8.into(), b"");
    endpoint_clone.wait_idle().await;

    Ok(())
}

async fn handle_connection(opt: Opt, conn: quinn::Connecting) -> Result<()> {
    let conn = conn.await?;

    let ssh_stream = tokio::net::TcpStream::connect(opt.forward).await?;
    let (ssh_recv, ssh_send) = tokio::io::split(ssh_stream);

    let bufsize = match opt.bufsize {
        Some(bufsize) => bufsize,
        None => u32::MAX,
    };
    let tx = handle_connection_tx(conn.clone(), ssh_recv, bufsize);
    let rx = handle_connection_rx(conn.clone(), ssh_send);

    tokio::select! {
        val = tx => val?,
        val = rx => val?,
    }
    log::debug!("Connection Closed");

    Ok(())
}

async fn handle_connection_tx(
    conn: quinn::Connection,
    ssh_recv: tokio::io::ReadHalf<tokio::net::TcpStream>,
    bufsize: u32,
) -> Result<()> {
    let q = Arc::new(Mutex::new(queue::Queue::new(bufsize)));
    let (quic_send, quic_recv) = conn.open_bi().await?;
    let tcp2quic = utils::pipe_tcp_to_quic(ssh_recv, quic_send, q.clone());
    let quicack = utils::consume_ack(q, quic_recv);

    tokio::select! {
        val = tcp2quic => val?,
        val = quicack => val?,
    }
    Ok(())
}

async fn handle_connection_rx(
    conn: quinn::Connection,
    std_send: tokio::io::WriteHalf<tokio::net::TcpStream>,
) -> Result<()> {
    let (quic_send, quic_recv) = conn.accept_bi().await?;
    let quic2tcp = utils::pipe_quic_to_tcp(quic_recv, quic_send, std_send);

    tokio::select! {
        val = quic2tcp => val?,
    }
    Ok(())
}
