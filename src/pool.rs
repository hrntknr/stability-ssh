use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ConnPool {
    conns: Arc<Mutex<std::collections::HashMap<Vec<u8>, Arc<Mutex<tokio::net::TcpStream>>>>>,
}

impl ConnPool {
    pub fn new() -> Self {
        Self {
            conns: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub async fn get(&mut self, pubkey: Vec<u8>) -> Option<Arc<Mutex<tokio::net::TcpStream>>> {
        let conns = self.conns.lock().await;
        conns.get(&pubkey).cloned()
    }

    pub async fn insert(
        &self,
        pubkey: Vec<u8>,
        conn: tokio::net::TcpStream,
    ) -> Option<Arc<Mutex<tokio::net::TcpStream>>> {
        let mut conns = self.conns.lock().await;
        conns.insert(pubkey.clone(), Arc::new(Mutex::new(conn)));
        conns.get(&pubkey).cloned()
    }

    pub async fn remove(&self, pubkey: Vec<u8>) {
        let mut conns = self.conns.lock().await;
        conns.remove(&pubkey);
    }
}
