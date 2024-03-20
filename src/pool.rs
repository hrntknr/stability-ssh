use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ConnPool<T: Clone> {
    conns: Arc<Mutex<std::collections::HashMap<Vec<u8>, T>>>,
}

impl<T: Clone> ConnPool<T> {
    pub fn new() -> Self {
        Self {
            conns: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub async fn get(&mut self, pubkey: Vec<u8>) -> Option<T> {
        let conns = self.conns.lock().await;
        conns.get(&pubkey).cloned()
    }

    pub async fn insert(&self, pubkey: Vec<u8>, conn: T) -> Option<T> {
        let mut conns = self.conns.lock().await;
        conns.insert(pubkey.clone(), conn);
        conns.get(&pubkey).cloned()
    }

    pub async fn remove(&self, pubkey: Vec<u8>) {
        let mut conns = self.conns.lock().await;
        conns.remove(&pubkey);
    }
}
