use crate::{queue, utils};
use anyhow::Result;
use clap::Parser;
use std::net::ToSocketAddrs;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Parser, Debug, Clone)]
#[clap(name = "client")]
pub struct Opt {
    target: String,

    #[clap(long = "ipv4", short = '4')]
    ipv4: bool,
    #[clap(long = "ipv6", short = '6')]
    ipv6: bool,

    #[clap(long = "bufsize", short = 'b')]
    bufsize: Option<u32>,
}

pub async fn run(opt: Opt) -> Result<()> {
    let targets = opt.target.to_socket_addrs()?;
    let targets = targets.filter(|addr| {
        if !opt.ipv4 && !opt.ipv6 {
            return true;
        }
        if opt.ipv4 {
            return addr.is_ipv4();
        }
        if opt.ipv6 {
            return addr.is_ipv6();
        }
        false
    });

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
    transport_config.max_idle_timeout(Some(Duration::from_secs(10).try_into()?));
    transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(1)));
    client_config.transport_config(Arc::new(transport_config));
    let mut endpoint = quinn::Endpoint::client("[::]:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    for target in targets.clone() {
        log::debug!("Connecting to {:?}", target);
        let conn = endpoint.connect(target, "localhost")?;
        handle_connection(opt, conn).await?;
        endpoint.wait_idle().await;
        return Ok(());
    }
    return Err(anyhow::anyhow!("target not found"));
}

async fn handle_connection(opt: Opt, conn: quinn::Connecting) -> Result<()> {
    let conn = conn.await?;

    let std_recv = tokio::io::BufReader::new(tokio::io::stdin());
    let std_send = tokio::io::BufWriter::new(tokio::io::stdout());

    let bufsize = match opt.bufsize {
        Some(bufsize) => bufsize,
        None => u32::MAX,
    };
    let tx = handle_connection_tx(conn.clone(), std_recv, bufsize);
    let rx = handle_connection_rx(conn.clone(), std_send);
    let signal_thread = utils::stop_signal_wait();

    tokio::select! {
        val = tx => val?,
        val = rx => val?,
        _ = signal_thread => {}
    }
    log::debug!("Connection Closed");

    Ok(())
}

async fn handle_connection_tx(
    conn: quinn::Connection,
    std_recv: tokio::io::BufReader<tokio::io::Stdin>,
    bufsize: u32,
) -> Result<()> {
    let q = Arc::new(Mutex::new(queue::Queue::new(bufsize)));
    let (quic_send, quic_recv) = conn.open_bi().await?;
    let std2quic = utils::pipe_std_to_quic(std_recv, quic_send, q.clone());
    let quicack = utils::consume_ack(q, quic_recv);

    tokio::select! {
        val = std2quic => val?,
        val = quicack => val?,
    }

    Ok(())
}
async fn handle_connection_rx(
    conn: quinn::Connection,
    std_send: tokio::io::BufWriter<tokio::io::Stdout>,
) -> Result<()> {
    let (quic_send, quic_recv) = conn.accept_bi().await?;
    let quic2std = utils::pipe_quic_to_std(quic_recv, quic_send, std_send);

    tokio::select! {
        val = quic2std => val?,
    }

    Ok(())
}
