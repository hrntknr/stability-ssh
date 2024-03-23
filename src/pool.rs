use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[derive(Clone)]
pub struct ConnPool {
    timer: tokio::time::Instant,
    hold_timeout: u64,
    conns: Arc<Mutex<std::collections::HashMap<Vec<u8>, ConnInfo>>>,
    last_active: Arc<Mutex<std::collections::HashMap<Vec<u8>, u64>>>,
}

#[derive(Clone)]
pub struct ConnInfo {
    pub conn: Arc<Mutex<tokio::net::TcpStream>>,
    pub q: Arc<Mutex<crate::queue::Queue>>,
    pub last_ack: Arc<RwLock<u32>>,
    pub name: Option<String>,
}

impl ConnInfo {
    pub fn new(
        conn: Arc<Mutex<tokio::net::TcpStream>>,
        q: Arc<Mutex<crate::queue::Queue>>,
        last_ack: Arc<RwLock<u32>>,
        name: Option<String>,
    ) -> Self {
        Self {
            conn,
            q,
            last_ack,
            name,
        }
    }
}

impl ConnPool {
    pub fn new(hold_timeout: u64) -> Self {
        Self {
            timer: tokio::time::Instant::now(),
            hold_timeout,
            conns: Arc::new(Mutex::new(std::collections::HashMap::new())),
            last_active: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn new_handle(&self, pubkey: Vec<u8>) -> ConnPoolHandle {
        ConnPoolHandle {
            timer: self.timer.clone(),
            pubkey,
            last_active: self.last_active.clone(),
        }
    }

    pub async fn get(&mut self, pubkey: Vec<u8>) -> Option<ConnInfo> {
        let conns = self.conns.lock().await;
        conns.get(&pubkey).cloned()
    }

    pub async fn insert(&self, pubkey: Vec<u8>, conn: ConnInfo) -> Option<ConnInfo> {
        let mut conns = self.conns.lock().await;
        conns.insert(pubkey.clone(), conn);
        conns.get(&pubkey).cloned()
    }

    pub async fn remove(&self, pubkey: Vec<u8>) {
        let mut conns = self.conns.lock().await;
        conns.remove(&pubkey);
    }

    pub async fn list(&self) -> Vec<Vec<u8>> {
        let conns = self.conns.lock().await;
        conns.keys().cloned().collect()
    }

    pub async fn kill(&self, pubkey: Vec<u8>) -> Result<bool> {
        let mut conns = self.conns.lock().await;
        let conn = conns.get(&pubkey).clone();
        if conn.is_some() {
            let conn = conn.unwrap().conn.try_lock();
            if conn.is_err() {
                return Ok(false);
            }
        }
        match conns.remove(&pubkey) {
            Some(_) => Ok(true),
            None => Err(anyhow::anyhow!("Connection not found")),
        }
    }

    pub async fn last_active(&self, pubkey: Vec<u8>) -> Option<u64> {
        let last_active = self.last_active.lock().await;
        let now = self.timer.elapsed().as_secs();
        match last_active.get(&pubkey) {
            Some(v) => Some(now - v),
            None => None,
        }
    }

    pub async fn qlen(&self, pubkey: Vec<u8>) -> Option<u32> {
        let conns = self.conns.lock().await;
        match conns.get(&pubkey) {
            Some(v) => Some(v.q.lock().await.len()),
            None => None,
        }
    }

    pub async fn hold(&self, pubkey: Vec<u8>) -> ConnPoolHandle {
        let mut last_active = self.last_active.lock().await;
        last_active.remove(&pubkey);
        self.new_handle(pubkey)
    }

    pub async fn collect(&self) {
        log::debug!("collect start");
        let mut last_active = self.last_active.lock().await;
        let now = self.timer.elapsed().as_secs();
        for (k, time) in last_active.clone().iter() {
            if now - time > self.hold_timeout {
                log::debug!("collect: {:?}", k);
                last_active.remove(k);
                self.conns.lock().await.remove(k);
            }
        }
    }
}

pub fn collect_loop(pool: ConnPool, interval: std::time::Duration) {
    let pool = pool.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            pool.collect().await;
        }
    });
}

#[derive(Debug)]
pub struct ConnPoolHandle {
    timer: tokio::time::Instant,
    pubkey: Vec<u8>,
    last_active: Arc<Mutex<std::collections::HashMap<Vec<u8>, u64>>>,
}

impl Drop for ConnPoolHandle {
    fn drop(&mut self) {
        let timer = self.timer.clone();
        let pubkey = self.pubkey.clone();
        let last_active = self.last_active.clone();
        tokio::spawn(async move {
            let mut last_active = last_active.lock().await;
            last_active.insert(pubkey, timer.elapsed().as_secs());
        });
    }
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_handle() {
        let pool = super::ConnPool::new(10);
        let pubkey = vec![1, 2, 3];
        let handle = pool.hold(pubkey.clone()).await;
        {
            let last_active = pool.last_active.lock().await;
            assert!(matches!(last_active.get(&pubkey), None));
        };
        drop(handle);
        tokio::time::sleep(tokio::time::Duration::from_millis(0)).await;
        {
            let last_active = pool.last_active.lock().await;
            assert!(matches!(last_active.get(&pubkey), Some(_)));
        };
    }
}
