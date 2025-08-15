use crate::config::{MAX_BACKFILL_PER_CYCLE, MAX_BLOCK_HISTORY, STALE_AFTER};
use alloy_provider::Provider as ProviderTrait;
use alloy_provider::RootProvider as AlloyProvider;
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

impl SignetMetrics {
    pub fn new(rpc_url: String, block_delay_threshold: u64) -> Self {
        Self {
            block_number: None,
            gas_price: None,
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
        })
    }
}

pub struct MetricsCollector {
    client: SignetRpcClient,
    metrics: SignetMetrics,
}

impl MetricsCollector {
    pub fn new(rpc_url: String, block_delay_threshold: u64) -> Self {
        let client = SignetRpcClient::new(rpc_url.clone()).unwrap();
        Self {
            client: client,
            metrics: SignetMetrics::new(rpc_url, block_delay_threshold),
        }
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

        self.metrics.connection_status = status;
        self.metrics.last_updated = Instant::now();
        if matches!(self.metrics.connection_status, ConnectionStatus::Connected) {
            self.metrics.last_successful = Some(self.metrics.last_updated);
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
