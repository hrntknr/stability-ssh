use crate::{pool, utils};
use anyhow::Result;
use clap::Parser;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{Mutex, RwLock};

#[derive(Parser, Debug, Clone)]
#[clap(name = "server")]
pub struct Opt {
    #[clap(long = "idle", short = 'i', default_value = "3")]
    idle: u64,

    #[clap(long = "keepalive", short = 'k', default_value = "1")]
    keepalive: u64,

    #[clap(long = "bufsize", short = 'b', default_value = "32")]
    bufsize: u8,

    #[clap(long = "listen", short = 'l', default_value = "0.0.0.0:2222")]
    listen: SocketAddr,

    #[clap(long = "forward", short = 'f', default_value = "localhost:22")]
    forward: String,
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
    if opt.idle > 0 {
        transport_config.max_idle_timeout(Some(Duration::from_secs(opt.idle).try_into()?));
    }
    if opt.keepalive > 0 {
        transport_config.keep_alive_interval(Some(Duration::from_secs(opt.keepalive)));
    }

    let endpoint = quinn::Endpoint::server(server_config, opt.listen)?;
    accept_loop(opt, endpoint.clone()).await?;

    endpoint.close(0_u8.into(), b"");
    endpoint.wait_idle().await;

    Ok(())
}

async fn accept_loop(opt: Opt, endpoint: quinn::Endpoint) -> Result<()> {
    let conn_pool = pool::ConnPool::new();
    tokio::spawn(async move {
        while let Some(conn) = endpoint.accept().await {
            let fut = handle_connection(opt.clone(), conn_pool.clone(), conn);
            tokio::spawn(async move {
                match fut.await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Connection error: {:?}", e);
                    }
                }
            });
        }
    });
    utils::stop_signal_wait().await;
    Ok(())
}

async fn handle_connection(
    opt: Opt,
    mut conn_pool: pool::ConnPool<(
        Arc<Mutex<tokio::net::TcpStream>>,
        Arc<RwLock<u32>>,
        Arc<RwLock<u32>>,
    )>,
    conn: quinn::Connecting,
) -> Result<()> {
    let conn = conn.await?;
    let pubkey = utils::x509pubkey(
        &conn
            .peer_identity()
            .unwrap()
            .downcast::<Vec<rustls::Certificate>>()
            .unwrap()
            .first()
            .unwrap(),
    )?;
    let (ssh_conn, tx_ack, rx_ack) = match conn_pool.get(pubkey.clone()).await {
        Some(v) => {
            log::debug!("Reusing connection for {:?}", pubkey);
            v
        }
        None => {
            log::debug!("Creating new connection for {:?}", pubkey);
            let ssh_conn = Arc::new(Mutex::new(
                tokio::net::TcpStream::connect(opt.forward).await?,
            ));
            let tx_ack = Arc::new(RwLock::new(0_u32));
            let rx_ack = Arc::new(RwLock::new(0_u32));
            conn_pool
                .insert(pubkey.clone(), (ssh_conn, tx_ack, rx_ack))
                .await
                .unwrap()
        }
    };

    let mut ssh_conn = ssh_conn.lock().await;
    let (ssh_recv, ssh_send) = ssh_conn.split();
    utils::handle_connection(opt.bufsize, conn, tx_ack, rx_ack, ssh_recv, ssh_send).await?;

    Ok(())
}
