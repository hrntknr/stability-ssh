use crate::{queue, utils};
use anyhow::Result;
use clap::Parser;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, RwLock};

#[derive(Parser, Debug, Clone)]
#[clap(name = "client")]
pub struct Opt {
    target: String,

    #[clap(long = "idle", short = 'i', default_value = "3")]
    idle: u64,

    #[clap(long = "keepalive", short = 'k', default_value = "1")]
    keepalive: u64,

    #[clap(long = "bufsize", short = 'b', default_value = "32")]
    bufsize: u8,

    #[clap(long = "only-ipv4", short = '4')]
    ipv4: bool,

    #[clap(long = "only-ipv6", short = '6')]
    ipv6: bool,
}

pub async fn run(opt: Opt) -> Result<()> {
    let (cert_der, priv_key) = crate::utils::gen_cert()?;
    let mut client_crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(crate::utils::SkipServerVerification::new())
        .with_client_auth_cert(
            vec![rustls::Certificate(cert_der.clone())],
            rustls::PrivateKey(priv_key),
        )?;
    client_crypto.alpn_protocols = vec![b"stablessh".to_vec()];
    let mut client_config = quinn::ClientConfig::new(Arc::new(client_crypto));
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.mtu_discovery_config(Some(quinn::MtuDiscoveryConfig::default()));
    if opt.idle > 0 {
        transport_config.max_idle_timeout(Some(Duration::from_secs(opt.idle).try_into()?));
    }
    if opt.keepalive > 0 {
        transport_config.keep_alive_interval(Some(Duration::from_secs(opt.keepalive)));
    }
    client_config.transport_config(Arc::new(transport_config));
    let mut endpoint = quinn::Endpoint::client("[::]:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    connect(opt, endpoint).await?;

    Ok(())
}

async fn connect(opt: Opt, endpoint: quinn::Endpoint) -> Result<()> {
    let mut std_recv = tokio::io::BufReader::new(tokio::io::stdin());
    let mut std_send = tokio::io::BufWriter::new(tokio::io::stdout());
    let q = Arc::new(Mutex::new(queue::Queue::new(opt.bufsize)));
    let last_ack = Arc::new(RwLock::new(0_u32));
    let targets = utils::resolve(&opt.target, opt.ipv4, opt.ipv6)?;
    'outer: loop {
        for target in targets.clone() {
            log::debug!("Connecting to {:?}", target);
            let conn = match endpoint.connect(target, "localhost") {
                Ok(conn) => conn,
                Err(_) => continue,
            };
            match handle_connection(
                conn,
                q.clone(),
                last_ack.clone(),
                &mut std_recv,
                &mut std_send,
            )
            .await
            {
                Ok(_) => return Ok(()),
                Err(e) => {
                    if is_retry(&e) {
                        continue 'outer;
                    }
                    if is_ok(&e) {
                        return Ok(());
                    }
                    return Err(e);
                }
            }
        }
        return Err(anyhow::anyhow!("target not found"));
    }
}

async fn handle_connection(
    conn: quinn::Connecting,
    q: Arc<Mutex<queue::Queue>>,
    last_ack: Arc<RwLock<u32>>,
    std_recv: &mut tokio::io::BufReader<tokio::io::Stdin>,
    std_send: &mut tokio::io::BufWriter<tokio::io::Stdout>,
) -> Result<()> {
    let conn = conn.await?;
    utils::handle_connection(conn, q, last_ack, std_recv, std_send).await?;
    Ok(())
}

fn is_ok(e: &anyhow::Error) -> bool {
    if matches!(
        e.downcast_ref(),
        Some(quinn::WriteError::ConnectionLost(
            quinn::ConnectionError::ApplicationClosed(_)
        ))
    ) {
        return true;
    }
    if matches!(
        e.downcast_ref(),
        Some(quinn::ReadError::ConnectionLost(
            quinn::ConnectionError::ApplicationClosed(_)
        ))
    ) {
        return true;
    }
    false
}

fn is_retry(e: &anyhow::Error) -> bool {
    if matches!(e.downcast_ref(), Some(quinn::ConnectionError::TimedOut)) {
        return true;
    }
    if matches!(e.downcast_ref(), Some(quinn::WriteError::ConnectionLost(_))) {
        return true;
    }
    if matches!(e.downcast_ref(), Some(quinn::ReadError::ConnectionLost(_))) {
        return true;
    }
    false
}
