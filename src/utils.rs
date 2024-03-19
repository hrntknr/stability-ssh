use crate::{pkt_buf, queue};
use anyhow::Result;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};
use x509_parser::der_parser::asn1_rs::FromDer;

const CHUNK_SIZE: usize = u16::MAX as usize;

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

pub fn resolve(target: &str, only4: bool, only6: bool) -> Result<Vec<SocketAddr>> {
    let targets = target.to_socket_addrs()?;
    log::debug!("Resolved targets: {:?}", targets);
    let targets = targets.filter(|addr| {
        if !only4 && !only6 {
            return true;
        }
        if only4 {
            return addr.is_ipv4();
        }
        if only6 {
            return addr.is_ipv6();
        }
        false
    });
    let targets = targets.collect::<Vec<SocketAddr>>();
    Ok(targets)
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

pub fn x509pubkey(cert: &rustls::Certificate) -> Result<Vec<u8>> {
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

pub async fn handle_connection<
    Reader: tokio::io::AsyncRead + Send + Sync + Unpin,
    Writer: tokio::io::AsyncWrite + Send + Sync + Unpin,
>(
    bufsize: u32,
    conn: quinn::Connection,
    recv: Reader,
    send: Writer,
) -> Result<()> {
    let tx = handle_connection_tx(conn.clone(), recv, bufsize);
    let rx = handle_connection_rx(conn.clone(), send);

    tokio::select! {
        val = tx => {val?;},
        val = rx => {val?;},
    }
    Ok(())
}

pub async fn handle_connection_tx<Reader: tokio::io::AsyncRead + Send + Sync + Unpin>(
    conn: quinn::Connection,
    recv: Reader,
    bufsize: u32,
) -> Result<()> {
    let q = Arc::new(Mutex::new(queue::Queue::new(bufsize)));
    let (quic_send, quic_recv) = conn.open_bi().await?;
    let reader2quic = pipe_reader_to_quic(recv, quic_send, q.clone());
    let ack = consume_ack(q, quic_recv);

    tokio::select! {
        val = reader2quic => val?,
        val = ack => val?,
    }
    Ok(())
}

pub async fn handle_connection_rx<Writer: tokio::io::AsyncWrite + Send + Sync + Unpin>(
    conn: quinn::Connection,
    send: Writer,
) -> Result<()> {
    let (quic_send, quic_recv) = conn.accept_bi().await?;
    let quic2writer = pipe_quic_to_writer(quic_recv, quic_send, send);

    tokio::select! {
        val = quic2writer => val?,
    }
    Ok(())
}

pub async fn pipe_quic_to_writer<Writer: tokio::io::AsyncWrite + Send + Sync + Unpin>(
    mut recv: quinn::RecvStream,
    mut ack: quinn::SendStream,
    mut send: Writer,
) -> Result<()> {
    let mut buf = [0; CHUNK_SIZE];
    let mut databuf = pkt_buf::DataBuf::new();
    loop {
        match recv.read(&mut buf).await? {
            None => break,
            Some(0) => break,
            Some(n) => {
                log::debug!("quic recv {} bytes", n);

                databuf.push(buf[..n].to_vec());
                loop {
                    match databuf.next() {
                        Some((id, d)) => {
                            send.write_all(&d).await?;
                            send.flush().await?;
                            ack.write_all(&pkt_buf::to_ack_pkt(id)).await?;
                        }
                        None => break,
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn pipe_reader_to_quic<Reader: tokio::io::AsyncRead + Send + Sync + Unpin>(
    mut recv: Reader,
    mut send: quinn::SendStream,
    q: Arc<Mutex<queue::Queue>>,
) -> Result<()> {
    let mut buf = [0; CHUNK_SIZE];
    loop {
        match recv.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                log::debug!("reader recv {} bytes", n);
                let id = q.lock().await.push(buf[..n].to_vec())?;
                let pkt = pkt_buf::to_pkt(id, buf[..n].to_vec());
                send.write_all(&pkt).await?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub async fn consume_ack(q: Arc<Mutex<queue::Queue>>, mut recv: quinn::RecvStream) -> Result<()> {
    let mut buf = [0; CHUNK_SIZE];
    let mut ackbuf = pkt_buf::AckBuf::new();
    loop {
        match recv.read(&mut buf).await? {
            None => break,
            Some(0) => break,
            Some(n) => {
                log::debug!("quic ack recv {} bytes", n);
                ackbuf.push(buf[..n].to_vec());
                loop {
                    match ackbuf.next() {
                        Some(id) => {
                            q.lock().await.check(id)?;
                        }
                        None => break,
                    }
                }
            }
        }
    }
    Ok(())
}
