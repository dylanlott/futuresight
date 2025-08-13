use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: Vec<Value>,
    id: u32,
}

impl RpcRequest {
    pub fn new(method: &str, params: Vec<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub id: u32,
}

#[derive(Debug, Clone)]
pub struct EthMetrics {
    pub block_number: Option<u64>,
    pub gas_price: Option<u64>,
    pub peer_count: Option<u32>,
    pub chain_id: Option<u64>,
    pub last_updated: Instant,
    pub last_successful: Option<Instant>,
    pub rpc_url: String,
    pub connection_status: ConnectionStatus,
    pub block_history: VecDeque<BlockInfo>,
    pub latest_block_timestamp: Option<u64>, // unix seconds
    pub block_delay_threshold: u64,          // seconds
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Stale,
    Disconnected,
    Error(String),
}

pub const STALE_AFTER: Duration = Duration::from_secs(20);
pub const MAX_BLOCK_HISTORY: usize = 20;
pub const MAX_BACKFILL_PER_CYCLE: u64 = 6;
pub const BLOCK_DELAY_DEFAULT: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub tx_count: usize,
    pub gas_used: u64,
    pub gas_limit: u64,
}

impl EthMetrics {
    pub fn new(rpc_url: String, block_delay_threshold: u64) -> Self {
        Self {
            block_number: None,
            gas_price: None,
            peer_count: None,
            chain_id: None,
            last_updated: Instant::now(),
            last_successful: None,
            rpc_url,
            connection_status: ConnectionStatus::Disconnected,
            block_history: VecDeque::with_capacity(MAX_BLOCK_HISTORY),
            latest_block_timestamp: None,
            block_delay_threshold,
        }
    }
}

pub struct EthRpcClient {
    client: reqwest::Client,
    rpc_url: String,
}

impl EthRpcClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            rpc_url,
        }
    }

    async fn call_rpc(&self, request: RpcRequest) -> Result<RpcResponse> {
        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await?
            .json::<RpcResponse>()
            .await?;
        Ok(response)
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        let request = RpcRequest::new("eth_blockNumber", vec![]);
        let response = self.call_rpc(request).await?;

        if let Some(result) = response.result {
            let hex_str = result.as_str().unwrap_or("0x0");
            let block_number = u64::from_str_radix(&hex_str[2..], 16)?;
            Ok(block_number)
        } else {
            anyhow::bail!("No result in response");
        }
    }

    pub async fn get_gas_price(&self) -> Result<u64> {
        let request = RpcRequest::new("eth_gasPrice", vec![]);
        let response = self.call_rpc(request).await?;

        if let Some(result) = response.result {
            let hex_str = result.as_str().unwrap_or("0x0");
            let gas_price = u64::from_str_radix(&hex_str[2..], 16)?;
            Ok(gas_price)
        } else {
            anyhow::bail!("No result in response");
        }
    }

    pub async fn get_peer_count(&self) -> Result<u32> {
        let request = RpcRequest::new("net_peerCount", vec![]);
        let response = self.call_rpc(request).await?;

        if let Some(result) = response.result {
            let hex_str = result.as_str().unwrap_or("0x0");
            let peer_count = u32::from_str_radix(&hex_str[2..], 16)?;
            Ok(peer_count)
        } else {
            anyhow::bail!("No result in response");
        }
    }

    pub async fn get_chain_id(&self) -> Result<u64> {
        let request = RpcRequest::new("eth_chainId", vec![]);
        let response = self.call_rpc(request).await?;

        if let Some(result) = response.result {
            let hex_str = result.as_str().unwrap_or("0x0");
            let chain_id = u64::from_str_radix(&hex_str.trim_start_matches("0x"), 16)?;
            Ok(chain_id)
        } else {
            anyhow::bail!("No result in response");
        }
    }

    pub async fn get_block_by_number(&self, number: u64) -> Result<BlockInfo> {
        let hex_number = format!("0x{:x}", number);
        let params = vec![Value::String(hex_number), Value::Bool(false)];
        let request = RpcRequest::new("eth_getBlockByNumber", params);
        let response = self.call_rpc(request).await?;

        if let Some(result) = response.result {
            let hash = result
                .get("hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let parent_hash = result
                .get("parentHash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let timestamp_hex = result
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            let gas_used_hex = result
                .get("gasUsed")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            let gas_limit_hex = result
                .get("gasLimit")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            let transactions = result
                .get("transactions")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let tx_count = transactions.len();

            let timestamp = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)?;
            let gas_used = u64::from_str_radix(gas_used_hex.trim_start_matches("0x"), 16)?;
            let gas_limit = u64::from_str_radix(gas_limit_hex.trim_start_matches("0x"), 16)?;

            Ok(BlockInfo {
                number,
                hash,
                parent_hash,
                timestamp,
                tx_count,
                gas_used,
                gas_limit,
            })
        } else {
            anyhow::bail!("No result in response");
        }
    }
}

pub struct MetricsCollector {
    client: EthRpcClient,
    metrics: EthMetrics,
}

impl MetricsCollector {
    pub fn new(rpc_url: String, block_delay_threshold: u64) -> Self {
        Self {
            client: EthRpcClient::new(rpc_url.clone()),
            metrics: EthMetrics::new(rpc_url, block_delay_threshold),
        }
    }

    pub async fn collect_metrics(&mut self) -> &EthMetrics {
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
                        while self.metrics.block_history.len() > MAX_BLOCK_HISTORY {
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

        // Peer count is considered informational only; its failure should not flip connection status.
        if let Ok(peer_count) = self.client.get_peer_count().await {
            self.metrics.peer_count = Some(peer_count);
        }

        self.metrics.connection_status = status;
        self.metrics.last_updated = Instant::now();
        if matches!(self.metrics.connection_status, ConnectionStatus::Connected) {
            self.metrics.last_successful = Some(self.metrics.last_updated);
        }
        &self.metrics
    }

    pub fn get_metrics(&self) -> &EthMetrics {
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
