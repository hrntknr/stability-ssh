use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use x509_parser::der_parser::asn1_rs::FromDer;

pub fn gen_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    let host: String = match hostname::get()?.into_string() {
        Ok(h) => h,
        Err(_) => "localhost".to_string(),
    };
    let cert = rcgen::generate_simple_self_signed(vec![host])?;
    let cert_der = cert.serialize_der()?;
    let priv_key = cert.serialize_private_key_der();
    Ok((cert_der, priv_key))
}

pub struct SkipClientVerification;

impl SkipClientVerification {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::server::ClientCertVerifier for SkipClientVerification {
    fn client_auth_root_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }
    fn verify_client_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _now: std::time::SystemTime,
    ) -> Result<rustls::server::ClientCertVerified, rustls::Error> {
        Ok(rustls::server::ClientCertVerified::assertion())
    }
}

pub struct SkipServerVerification;

impl SkipServerVerification {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

pub fn x509pubkey(cert: rustls::Certificate) -> Result<Vec<u8>> {
    let (_, peer) = x509_parser::prelude::X509Certificate::from_der(&cert.0)?;
    Ok(peer.public_key().subject_public_key.data.to_vec())
}

pub async fn stop_signal_wait() {
    tokio::select! {
        _ = signal(tokio::signal::unix::SignalKind::hangup()) => {}
        _ = signal(tokio::signal::unix::SignalKind::interrupt()) => {}
        _ = signal(tokio::signal::unix::SignalKind::terminate()) => {}
    }
}

async fn signal(sig: tokio::signal::unix::SignalKind) {
    let mut f = match tokio::signal::unix::signal(sig) {
        Ok(s) => s,
        Err(e) => {
            log::error!("signal: {:?}", e);
            return;
        }
    };
    f.recv().await;
}

pub async fn pipe_quic_to_tcp(
    mut recv: quinn::RecvStream,
    mut send: tokio::io::WriteHalf<tokio::net::TcpStream>,
) {
    let mut buf = [0; 2048];
    loop {
        match recv.read(&mut buf).await {
            Ok(None) => break,
            Ok(Some(0)) => break,
            Ok(Some(n)) => {
                log::debug!("quic recv {} bytes", n);
                if let Err(e) = send.write_all(&buf[..n]).await {
                    log::error!("pipe_quic_to_tcp: {:?}", e);
                    break;
                }
            }
            Err(quinn::ReadError::ConnectionLost(_)) => {
                break;
            }
            Err(e) => {
                log::error!("pipe_quic_to_tcp: {:?}", e);
                break;
            }
        }
    }
}

pub async fn pipe_tcp_to_quic(
    mut recv: tokio::io::ReadHalf<tokio::net::TcpStream>,
    mut send: quinn::SendStream,
) {
    let mut buf = [0; 2048];
    loop {
        match recv.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                log::debug!("tcp recv {} bytes", n);
                if let Err(e) = send.write_all(&buf[..n]).await {
                    log::error!("pipe_tcp_to_quic: {:?}", e);
                    break;
                }
            }
            Err(e) => {
                log::error!("pipe_tcp_to_quic: {:?}", e);
                break;
            }
        }
    }
}

pub async fn pipe_quic_to_std(
    mut recv: quinn::RecvStream,
    mut send: tokio::io::BufWriter<tokio::io::Stdout>,
) {
    let mut buf = [0; 2048];
    loop {
        match recv.read(&mut buf).await {
            Ok(None) => break,
            Ok(Some(0)) => break,
            Ok(Some(n)) => {
                log::debug!("quic recv {} bytes", n);
                if let Err(e) = send.write_all(&buf[..n]).await {
                    log::error!("pipe_quic_to_std: {:?}", e);
                    break;
                }
                if let Err(e) = send.flush().await {
                    log::error!("pipe_quic_to_std: {:?}", e);
                    break;
                }
            }
            Err(quinn::ReadError::ConnectionLost(_)) => {
                break;
            }
            Err(e) => {
                log::error!("pipe_quic_to_std: {:?}", e);
                break;
            }
        }
    }
}

pub async fn pipe_std_to_quic(
    mut recv: tokio::io::BufReader<tokio::io::Stdin>,
    mut send: quinn::SendStream,
) {
    let mut buf = [0; 2048];
    loop {
        match recv.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                log::debug!("std recv {} bytes", n);
                if let Err(e) = send.write_all(&buf[..n]).await {
                    log::error!("pipe_std_to_quic: {:?}", e);
                    break;
                }
            }
            Err(e) => {
                log::error!("pipe_std_to_quic: {:?}", e);
                break;
            }
        }
    }
}
