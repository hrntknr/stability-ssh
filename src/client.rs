use crate::utils;
use anyhow::Result;
use clap::Parser;
use std::net::ToSocketAddrs;
use std::{sync::Arc, time::Duration};

#[derive(Parser, Debug, Clone)]
#[clap(name = "client")]
pub struct Opt {
    target: String,

    #[clap(long = "ipv4", short = '4')]
    ipv4: bool,
    #[clap(long = "ipv6", short = '6')]
    ipv6: bool,

    #[clap(long = "bufsize", short = 'b')]
    bufsize: Option<usize>,
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
        return handle_connection(opt, conn).await;
    }
    return Err(anyhow::anyhow!("target not found"));
}

async fn handle_connection(_opt: Opt, conn: quinn::Connecting) -> Result<()> {
    let conn = conn.await?;

    let std_recv = tokio::io::BufReader::new(tokio::io::stdin());
    let std_send = tokio::io::BufWriter::new(tokio::io::stdout());

    let (quic_send, quic_recv) = conn.accept_bi().await?;

    let quic2tcp = utils::pipe_quic_to_std(quic_recv, std_send);
    let tcp2quic = utils::pipe_std_to_quic(std_recv, quic_send);
    let signal_thread = utils::stop_signal_wait();

    tokio::select! {
        _ = quic2tcp => {}
        _ = tcp2quic => {}
        _ = signal_thread => {}
    }
    log::debug!("Connection Closed");

    Ok(())
}
