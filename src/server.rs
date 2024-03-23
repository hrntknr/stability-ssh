use crate::{pool, proto_impl, queue, utils};
use anyhow::Result;
use clap::Parser;
use core::time;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{Mutex, RwLock};

#[derive(Parser, Debug, Clone)]
#[clap(name = "server")]
pub struct Opt {
    #[clap(long = "idle", short = 'i', default_value = "3")]
    idle: u64,

    #[clap(long = "keepalive", short = 'k', default_value = "1")]
    keepalive: u64,

    #[clap(long = "bufsize", short = 'b', default_value = "18")]
    bufsize: u8,

    #[clap(long = "hold-timeout", short = 't', default_value = "604800")]
    hold_timeout: u64,

    #[clap(long = "hold-collect-interval", short = 'c', default_value = "60")]
    hold_collect_interval: u64,

    #[clap(long = "listen", short = 'l', default_value = "[::]:2222")]
    listen: SocketAddr,

    #[clap(long = "forward", short = 'f', default_value = "localhost:22")]
    forward: String,

    #[clap(long = "ctl-listen", default_value = "[::1]:50051")]
    ctl_listen: SocketAddr,
}

pub async fn run(opt: Opt) -> Result<()> {
    let conn_pool = pool::ConnPool::new(opt.hold_timeout);
    pool::collect_loop(
        conn_pool.clone(),
        time::Duration::from_secs(opt.hold_collect_interval),
    );
    let ret = tokio::select! {
        ret = server(opt.clone(), conn_pool.clone()) => ret,
        ret = grpc_server(opt.clone(), conn_pool.clone()) => ret,
    };

    ret?;
    Ok(())
}

async fn grpc_server(opt: Opt, pool: pool::ConnPool) -> Result<()> {
    tonic::transport::Server::builder()
        .add_service(crate::proto::ctl_service_server::CtlServiceServer::new(
            proto_impl::CtlServiceImpl::new(pool),
        ))
        .serve(opt.ctl_listen)
        .await?;

    Ok(())
}

pub async fn server(opt: Opt, pool: pool::ConnPool) -> Result<()> {
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
    accept_loop(opt, endpoint.clone(), pool).await?;

    endpoint.close(0_u8.into(), b"");
    endpoint.wait_idle().await;

    Ok(())
}

async fn accept_loop(opt: Opt, endpoint: quinn::Endpoint, pool: pool::ConnPool) -> Result<()> {
    tokio::spawn(async move {
        while let Some(conn) = endpoint.accept().await {
            let fut = handle_connection(opt.clone(), pool.clone(), conn);
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
    mut conn_pool: pool::ConnPool,
    conn: quinn::Connecting,
) -> Result<()> {
    let conn = conn.await?;
    let (pubkey, name) = utils::x509(
        &conn
            .peer_identity()
            .unwrap()
            .downcast::<Vec<rustls::Certificate>>()
            .unwrap()
            .first()
            .unwrap(),
    )?;
    let conn_info = match conn_pool.get(pubkey.clone()).await {
        Some(v) => {
            log::debug!("Reusing connection for {:?}", pubkey);
            v
        }
        None => {
            log::debug!("Creating new connection for {:?}", pubkey);
            let ssh_conn = Arc::new(Mutex::new(
                tokio::net::TcpStream::connect(opt.forward).await?,
            ));
            let q = Arc::new(Mutex::new(queue::Queue::new(opt.bufsize)));
            let last_ack = Arc::new(RwLock::new(0_u32));

            conn_pool
                .insert(
                    pubkey.clone(),
                    pool::ConnInfo::new(ssh_conn, q, last_ack, name),
                )
                .await
                .unwrap()
        }
    };

    let mut ssh_conn = conn_info.conn.lock().await;
    let (ssh_recv, ssh_send) = ssh_conn.split();
    let _handle = conn_pool.hold(pubkey.clone()).await;
    utils::handle_connection(conn, conn_info.q, conn_info.last_ack, ssh_recv, ssh_send).await?;

    Ok(())
}
