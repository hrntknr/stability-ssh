use crate::{pool, proto, utils};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct CtlServiceImpl {
    pool: Arc<Mutex<pool::ConnPool>>,
}

impl CtlServiceImpl {
    pub fn new(pool: pool::ConnPool) -> Self {
        Self {
            pool: Arc::new(Mutex::new(pool)),
        }
    }
}

#[tonic::async_trait]
impl proto::ctl_service_server::CtlService for CtlServiceImpl {
    async fn conn_list(
        &self,
        _req: tonic::Request<proto::ConnListRequest>,
    ) -> Result<tonic::Response<proto::ConnListResponse>, tonic::Status> {
        let mut pool = self.pool.lock().await;
        let pubkeys = pool.list().await;

        let mut res = proto::ConnListResponse::default();
        for pubkey in pubkeys {
            let mut res_info = proto::ConnInfo::default();
            let info = pool.get(pubkey.clone()).await;
            res_info.id = utils::pubkey_to_id(&pubkey);
            res_info.name = info.unwrap().name;
            res_info.last_active = pool.last_active(pubkey.clone()).await;
            res_info.pkt_buf = pool.qlen(pubkey.clone()).await;

            res.conns.push(res_info);
        }

        Ok(tonic::Response::new(res))
    }
    async fn conn_kill(
        &self,
        _req: tonic::Request<proto::ConnKillRequest>,
    ) -> Result<tonic::Response<proto::ConnKillResponse>, tonic::Status> {
        let pool = self.pool.lock().await;
        let pubkeys = pool.list().await;
        let pubkey = pubkeys
            .iter()
            .find(|&k| utils::pubkey_to_id(k) == _req.get_ref().id);
        if pubkey.is_none() {
            return Err(tonic::Status::not_found("Connection not found"));
        }
        match pool.kill(pubkey.unwrap().clone()).await {
            Ok(true) => Ok(tonic::Response::new(proto::ConnKillResponse {})),
            Ok(false) => Err(tonic::Status::internal("Connection is in use")),
            Err(e) => Err(tonic::Status::internal(e.to_string())),
        }
    }
}

pub struct CtlClient {
    client: proto::ctl_service_client::CtlServiceClient<tonic::transport::Channel>,
}

impl CtlClient {
    pub async fn new(uri: &str) -> Result<Self> {
        let client = proto::ctl_service_client::CtlServiceClient::connect(uri.to_string()).await?;
        Ok(Self { client })
    }

    pub async fn conn_list(&mut self) -> Result<proto::ConnListResponse, tonic::Status> {
        self.client
            .conn_list(proto::ConnListRequest {})
            .await
            .map(|r| r.into_inner())
    }

    pub async fn conn_kill(
        &mut self,
        conn_id: &str,
    ) -> Result<proto::ConnKillResponse, tonic::Status> {
        self.client
            .conn_kill(proto::ConnKillRequest {
                id: conn_id.to_string(),
            })
            .await
            .map(|r| r.into_inner())
    }
}
