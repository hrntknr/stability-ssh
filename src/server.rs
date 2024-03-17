use crate::utils;
use anyhow::Result;
use clap::Parser;
use std::{net::SocketAddr, sync::Arc, time::Duration};

#[derive(Parser, Debug, Clone)]
#[clap(name = "server")]
pub struct Opt {
    #[clap(long = "listen", short = 'l', default_value = "0.0.0.0:2222")]
    listen: SocketAddr,

    #[clap(long = "forward", short = 'f', default_value = "localhost:22")]
    forward: String,

    #[clap(long = "bufsize", short = 'b')]
    bufsize: Option<usize>,
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

    let (quic_send, quic_recv) = conn.open_bi().await?;

    let quic2tcp = utils::pipe_quic_to_tcp(quic_recv, ssh_send);
    let tcp2quic = utils::pipe_tcp_to_quic(ssh_recv, quic_send);

    tokio::select! {
        _ = quic2tcp => {}
        _ = tcp2quic => {}
    }
    log::debug!("Connection Closed");

    Ok(())
}
