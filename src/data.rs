use crate::config::{MAX_BACKFILL_PER_CYCLE, STALE_AFTER};
use alloy::eips::eip4844::BlobTransactionSidecarItem;
use alloy_provider::{Provider as ProviderTrait, RootProvider as AlloyProvider};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;
use url::Url;

#[derive(Debug, Clone)]
pub struct SignetMetrics {
    pub block_number: Option<u64>,
    pub gas_price: Option<u128>,
    pub chain_id: Option<u64>,
    pub last_updated: Instant,
    pub last_successful: Option<Instant>,
    pub rpc_url: String,
    pub connection_status: ConnectionStatus,
    pub block_history: VecDeque<BlockInfo>,
    pub max_block_history: usize,
    pub latest_block_timestamp: Option<u64>, // unix seconds
    pub block_delay_threshold: u64,          // seconds
    pub txpool: Option<TxPoolMetrics>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Stale,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub tx_count: usize,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub blobs: Vec<BlobTransactionSidecarItem>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub block_delay_threshold: u64,
    pub max_block_history: usize,
}

impl SignetMetrics {
    pub fn new(config: Config) -> Self {
        Self {
            block_number: None,
            gas_price: None,
            chain_id: None,
            last_updated: Instant::now(),
            last_successful: None,
            rpc_url: config.rpc_url,
            connection_status: ConnectionStatus::Disconnected,
            block_history: VecDeque::with_capacity(config.max_block_history),
            max_block_history: config.max_block_history,
            latest_block_timestamp: None,
            block_delay_threshold: config.block_delay_threshold,
            txpool: None,
        }
    }
}

pub struct SignetRpcClient {
    provider: AlloyProvider,
}

impl SignetRpcClient {
    pub fn new(rpc_url: String) -> Result<Self> {
        let url = Url::parse(&rpc_url)?;
        let provider = AlloyProvider::new_http(url);
        Ok(Self { provider })
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number)
    }

    pub async fn get_gas_price(&self) -> Result<u128> {
        let gas = self.provider.get_gas_price().await?;
        Ok(gas)
    }

    pub async fn get_chain_id(&self) -> Result<u64> {
        let id = self.provider.get_chain_id().await?;
        Ok(id)
    }

    pub async fn get_block_by_number(&self, number: u64) -> Result<BlockInfo> {
        // Use alloy provider's get_block API which returns Option<Block>
        let block = self
            .provider
            .get_block_by_number(alloy::eips::BlockNumberOrTag::Number(number))
            .await?
            .ok_or_else(|| eyre::eyre!("block not found"))?;

        Ok(BlockInfo {
            number: block.number(),
            hash: block.hash().to_string(),
            parent_hash: block.header.parent_hash.to_string(),
            timestamp: block.header.timestamp,
            tx_count: block.transactions.len(),
            gas_used: block.header.gas_used,
            gas_limit: block.header.gas_limit,
            blobs: vec![],
        })
    }
}

pub struct MetricsCollector {
    client: SignetRpcClient,
    metrics: SignetMetrics,
    tx_client: Option<TxPoolClient>,
}

impl MetricsCollector {
    pub fn new(config: Config) -> Self {
        let client = SignetRpcClient::new(config.rpc_url.clone()).unwrap();
        Self {
            client: client,
            metrics: SignetMetrics::new(Config {
                rpc_url: config.rpc_url,
                block_delay_threshold: config.block_delay_threshold,
                max_block_history: config.max_block_history,
            }),
            tx_client: None,
        }
    }

    /// Construct a collector with an optional tx-pool-webservice base URL.
    pub fn new_with_txpool(config: Config, txpool_url: Option<String>) -> Self {
        let mut s = Self::new(config);
        if let Some(url) = txpool_url {
            s.tx_client = Some(TxPoolClient::new(url));
        } else {
            s.tx_client = Some(TxPoolClient::new(
                "https://transactions.pecorino.signet.sh/".to_string(),
            ));
        }
        s
    }

    pub async fn collect_metrics(&mut self) -> &SignetMetrics {
        // Determine connectivity primarily via chain_id (lightweight) then block number/gas price.
        let mut status = match self.client.get_chain_id().await {
            Ok(chain_id) => {
                self.metrics.chain_id = Some(chain_id);
                ConnectionStatus::Connected
            }
            Err(e) => ConnectionStatus::Error(format!("Chain ID: {}", e)),
        };

        if matches!(status, ConnectionStatus::Connected) {
            if let Err(e) = self
                .client
                .get_block_number()
                .await
                .map(|b| self.metrics.block_number = Some(b))
            {
                status = ConnectionStatus::Error(format!("Block number: {}", e));
            }
        }

        // Maintain block history if we have a block number and are connected or stale.
        if matches!(status, ConnectionStatus::Connected) {
            if let Some(latest_num) = self.metrics.block_number {
                let last_recorded = self.metrics.block_history.front().map(|b| b.number); // front as newest

                // Determine range to fetch (inclusive) ensuring descending storage (newest at front)
                let fetch_range: Vec<u64> = if let Some(last) = last_recorded {
                    if latest_num > last {
                        let start = last + 1;
                        let end = latest_num;
                        (start..=end)
                            .rev()
                            .take(MAX_BACKFILL_PER_CYCLE as usize)
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    vec![latest_num]
                };

                for num in fetch_range {
                    // numbers already reversed for newest-first insert
                    if let Ok(block) = self.client.get_block_by_number(num).await {
                        let ts = block.timestamp;
                        if self
                            .metrics
                            .latest_block_timestamp
                            .map(|cur| ts > cur)
                            .unwrap_or(true)
                        {
                            self.metrics.latest_block_timestamp = Some(ts);
                        }
                        self.metrics.block_history.push_front(block);
                        while self.metrics.block_history.len() > self.metrics.max_block_history {
                            self.metrics.block_history.pop_back();
                        }
                    }
                }
            }
        }

        if matches!(status, ConnectionStatus::Connected) {
            if let Err(e) = self
                .client
                .get_gas_price()
                .await
                .map(|g| self.metrics.gas_price = Some(g))
            {
                status = ConnectionStatus::Error(format!("Gas price: {}", e));
            }
        }

        self.metrics.connection_status = status;
        self.metrics.last_updated = Instant::now();
        if matches!(self.metrics.connection_status, ConnectionStatus::Connected) {
            self.metrics.last_successful = Some(self.metrics.last_updated);
        }
        // Tx-pool metrics (best-effort, does not affect RPC status)
        if let Some(client) = &self.tx_client {
            match client.fetch_metrics().await {
                Ok(txm) => self.metrics.txpool = Some(txm),
                Err(e) => {
                    self.metrics.txpool = Some(TxPoolMetrics::with_error(e.to_string()));
                }
            }
        }
        &self.metrics
    }

    pub fn get_metrics(&self) -> &SignetMetrics {
        &self.metrics
    }

    pub fn check_staleness(&mut self) {
        if matches!(
            self.metrics.connection_status,
            ConnectionStatus::Connected | ConnectionStatus::Stale
        ) {
            if let Some(last_ok) = self.metrics.last_successful {
                if last_ok.elapsed() > STALE_AFTER {
                    self.metrics.connection_status = ConnectionStatus::Stale;
                }
            }
        }
    }
}

// ========================= TX-POOL SUPPORT =========================

#[derive(Debug, Clone)]
pub struct TxPoolMetrics {
    pub healthy: bool,
    pub last_updated: Instant,
    pub error: Option<String>,
    pub base_url: String,
    pub transactions_cache: Option<u64>,
    pub bundles_cache: Option<u64>,
    pub signed_orders_cache: Option<u64>,
}

impl TxPoolMetrics {
    pub fn new(base_url: String) -> Self {
        Self {
            healthy: false,
            last_updated: Instant::now(),
            error: None,
            base_url,
            transactions_cache: None,
            bundles_cache: None,
            signed_orders_cache: None,
        }
    }

    pub fn with_error(err: String) -> Self {
        Self {
            healthy: false,
            last_updated: Instant::now(),
            error: Some(err),
            base_url: "".to_string(),
            transactions_cache: None,
            bundles_cache: None,
            signed_orders_cache: None,
        }
    }
}

pub struct TxPoolClient {
    base_url: String,
    http: reqwest::Client,
}

impl TxPoolClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            http: reqwest::Client::new(),
        }
    }

    fn join_url(&self, path: &str) -> String {
        format!(
            "{}{}{}",
            self.base_url.trim_end_matches('/'),
            "/",
            path.trim_start_matches('/')
        )
    }

    async fn fetch_count_from(&self, path: &str) -> Result<Option<u64>> {
        let url = self.join_url(path);
        let resp = self.http.get(&url).send().await?;

        if !resp.status().is_success() {
            return Ok(None);
        }
        let body = resp.text().await?;
        let json: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        Ok(count_items(&json))
    }

    pub async fn fetch_metrics(&self) -> Result<TxPoolMetrics> {
        let mut out = TxPoolMetrics::new(self.base_url.clone());
        out.last_updated = Instant::now();

        // Fetch counts from endpoints. Do sequentially for simplicity/reliability.
        // Endpoints: /transactions, /bundles, /signed-orders
        let mut errors: Vec<String> = Vec::new();

        match self.fetch_count_from("/transactions").await {
            Ok(v) => out.transactions_cache = v,
            Err(e) => errors.push(format!("transactions: {}", e)),
        }

        match self.fetch_count_from("/bundles").await {
            Ok(v) => out.bundles_cache = v,
            Err(e) => errors.push(format!("bundles: {}", e)),
        }

        match self.fetch_count_from("/signed-orders").await {
            Ok(v) => out.signed_orders_cache = v,
            Err(e) => errors.push(format!("signed-orders: {}", e)),
        }

        if errors.is_empty() {
            out.healthy = true;
        } else {
            out.healthy = out.transactions_cache.is_some()
                || out.bundles_cache.is_some()
                || out.signed_orders_cache.is_some();
            out.error = Some(errors.join(", "));
        }

        Ok(out)
    }
}

fn count_items(v: &serde_json::Value) -> Option<u64> {
    // Accept common container shapes
    if let Some(arr) = v.as_array() {
        return Some(arr.len() as u64);
    }
    if let Some(obj) = v.as_object() {
        // Look for "items", "data", or pluralized keys
        for key in [
            "items",
            "data",
            "transactions",
            "bundles",
            "signedOrders",
            "signed_orders",
        ] {
            if let Some(arr) = obj.get(key).and_then(|x| x.as_array()) {
                return Some(arr.len() as u64);
            }
        }
    }
    None
}
