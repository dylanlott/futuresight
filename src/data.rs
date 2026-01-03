use crate::config::{
    FEE_HISTORY_BLOCKS, FEE_HISTORY_PERCENTILES, MAX_BACKFILL_PER_CYCLE, STALE_AFTER,
    SUGGESTION_RAMP_FACTOR,
};
use alloy::eips::eip4844::BlobTransactionSidecarItem;
use alloy::primitives::{Address, B256, U256};
use alloy_provider::{Provider as ProviderTrait, RootProvider as AlloyProvider};
use eyre::Result;
use reqwest;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, str::FromStr, time::Instant};
use url::Url;

// ========================= GAS TRACKING HELPERS =========================

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct SuggestedFeeTier {
    pub max_fee_per_gas: u128,          // wei
    pub max_priority_fee_per_gas: u128, // wei
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct SuggestedFees {
    pub safe: SuggestedFeeTier,
    pub standard: SuggestedFeeTier,
    pub fast: SuggestedFeeTier,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct FeeHistoryMetrics {
    pub oldest_block: u64,
    pub block_count: u64,
    pub base_fees: Vec<u128>,                     // wei per block
    pub gas_used_ratios: Vec<f64>,                // 0..=100 per block
    pub reward_percentiles: Vec<(u8, Vec<u128>)>, // (percentile, rewards per block in wei)
}

// ========================= METRICS MODEL =========================

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

    // Gas tracking (EIP-1559)
    pub base_fee_per_gas: Option<u128>,           // wei
    pub next_base_fee_per_gas: Option<u128>,      // wei
    pub max_priority_fee_suggested: Option<u128>, // wei
    pub suggested_fees: Option<SuggestedFees>,
    pub fee_history: Option<FeeHistoryMetrics>,
    pub gas_utilization_ma_n: Option<f64>, // percent 0..=100
    pub gas_volatility_5m: Option<f64>,    // relative pct vs MA

    // EIP-4844 (optional)
    #[allow(dead_code)]
    pub blob_base_fee: Option<u128>,
    #[allow(dead_code)]
    pub blob_base_fee_next: Option<u128>,
    #[allow(dead_code)]
    pub blob_gas_utilization_ma_n: Option<f64>,
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

    // header-derived gas fields
    pub base_fee_per_gas: Option<u128>, // 1559
    pub blob_gas_used: Option<u64>,     // 4844 (if available)
    pub excess_blob_gas: Option<u64>,   // 4844 (if available)
}

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    pub block_delay_threshold: u64,
    pub max_block_history: usize,
    pub txpool_max_rows: usize,
    pub txpool_fetch_list: bool,
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

            // init gas fields
            base_fee_per_gas: None,
            next_base_fee_per_gas: None,
            max_priority_fee_suggested: None,
            suggested_fees: None,
            fee_history: None,
            gas_utilization_ma_n: None,
            gas_volatility_5m: None,
            blob_base_fee: None,
            blob_base_fee_next: None,
            blob_gas_utilization_ma_n: None,
        }
    }
}

pub struct SignetRpcClient {
    provider: AlloyProvider,
    rpc_url: String,
    http: reqwest::Client,
}

impl SignetRpcClient {
    pub fn new(rpc_url: String) -> Result<Self> {
        let url = Url::parse(&rpc_url)?;
        let provider = AlloyProvider::new_http(url);
        Ok(Self {
            provider,
            rpc_url,
            http: reqwest::Client::new(),
        })
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

        // Best-effort header-derived gas fields (may be None on pre-1559/4844)
        let base_fee_per_gas = block.header.base_fee_per_gas.map(|v| v as u128);
        let blob_gas_used = None;
        let excess_blob_gas = None;

        Ok(BlockInfo {
            number: block.number(),
            hash: block.hash().to_string(),
            parent_hash: block.header.parent_hash.to_string(),
            timestamp: block.header.timestamp,
            tx_count: block.transactions.len(),
            gas_used: block.header.gas_used,
            gas_limit: block.header.gas_limit,
            blobs: vec![],

            base_fee_per_gas,
            blob_gas_used,
            excess_blob_gas,
        })
    }

    pub async fn get_fee_history(
        &self,
        block_count: u64,
        newest: &str,               // "latest" or hex
        reward_percentiles: &[f64], // 0..=100
    ) -> Result<EthFeeHistoryResult> {
        let params = serde_json::json!([to_hex_qty(block_count), newest, reward_percentiles]);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_feeHistory",
            "params": params
        });
        let resp = self.http.post(&self.rpc_url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(eyre::eyre!(format!(
                "eth_feeHistory HTTP {}",
                resp.status()
            )));
        }
        let v: serde_json::Value = resp.json().await?;
        if let Some(err) = v.get("error") {
            return Err(eyre::eyre!(format!("eth_feeHistory error: {}", err)));
        }
        let res: EthFeeHistoryResult = serde_json::from_value(v["result"].clone())?;
        Ok(res)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthFeeHistoryResult {
    pub oldest_block: String,
    #[serde(default)]
    pub base_fee_per_gas: Vec<String>, // hex; length blockCount+1
    #[serde(default)]
    pub gas_used_ratio: Vec<f64>, // 0..=1 length blockCount
    #[serde(default)]
    pub reward: Vec<Vec<String>>, // [blockCount][percentiles]
}

fn to_hex_qty(n: u64) -> String {
    format!("0x{:x}", n)
}
fn hex_to_u64(s: &str) -> Option<u64> {
    let t = s.trim_start_matches("0x");
    u64::from_str_radix(t, 16).ok()
}
fn hex_to_u128(s: &str) -> Option<u128> {
    let t = s.trim_start_matches("0x");
    u128::from_str_radix(t, 16).ok()
}

pub struct MetricsCollector {
    client: SignetRpcClient,
    metrics: SignetMetrics,
    tx_client: Option<TxPoolClient>,
}

impl MetricsCollector {
    pub fn new(config: Config) -> Self {
        let client = SignetRpcClient::new(config.rpc_url.clone()).unwrap();
        let metrics = SignetMetrics::new(config.clone());
        Self {
            client,
            metrics,
            tx_client: None,
        }
    }

    /// Construct a collector with an optional tx-pool-webservice base URL.
    pub fn new_with_txpool(config: Config, txpool_url: Option<String>) -> Self {
        let max_rows = config.txpool_max_rows;
        let fetch_list = config.txpool_fetch_list;
        let mut s = Self::new(config);
        let url = txpool_url.unwrap_or_else(|| "https://transactions.parmigiana.signet.sh/".to_string());
        s.tx_client = Some(TxPoolClient::new(url, max_rows, fetch_list));
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

            // Fee history polling (best-effort)
            match self
                .client
                .get_fee_history(FEE_HISTORY_BLOCKS, "latest", &FEE_HISTORY_PERCENTILES)
                .await
            {
                Ok(h) => {
                    // parse base fees and next base
                    let mut base_fees: Vec<u128> = Vec::with_capacity(h.base_fee_per_gas.len());
                    for hf in &h.base_fee_per_gas {
                        if let Some(v) = hex_to_u128(hf) {
                            base_fees.push(v);
                        }
                    }
                    let (current_base_fee, next_base_fee) = if base_fees.len() >= 2 {
                        (
                            Some(base_fees[base_fees.len() - 2]),
                            Some(base_fees[base_fees.len() - 1]),
                        )
                    } else {
                        (base_fees.last().copied(), None)
                    };
                    self.metrics.base_fee_per_gas = current_base_fee;
                    self.metrics.next_base_fee_per_gas = next_base_fee;

                    // gas used ratios -> percent
                    let gas_used_ratios: Vec<f64> =
                        h.gas_used_ratio.iter().map(|r| r * 100.0).collect();
                    let utilization_ma = if gas_used_ratios.is_empty() {
                        None
                    } else {
                        Some(gas_used_ratios.iter().sum::<f64>() / gas_used_ratios.len() as f64)
                    };
                    self.metrics.gas_utilization_ma_n = utilization_ma;

                    // volatility: latest vs avg (exclude next)
                    self.metrics.gas_volatility_5m = if base_fees.len() >= 2 {
                        let sample = &base_fees[..base_fees.len() - 1];
                        let avg =
                            sample.iter().map(|&x| x as f64).sum::<f64>() / sample.len() as f64;
                        let cur = sample.last().copied().unwrap_or(0) as f64;
                        if avg > 0.0 {
                            Some((cur - avg) / avg)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // reward percentiles -> series
                    let block_count = h.gas_used_ratio.len() as u64;
                    let mut reward_perc: Vec<(u8, Vec<u128>)> = Vec::new();
                    for (pi, pct) in FEE_HISTORY_PERCENTILES.iter().enumerate() {
                        let mut series: Vec<u128> = Vec::with_capacity(h.reward.len());
                        for row in &h.reward {
                            if let Some(hexv) = row.get(pi) {
                                if let Some(v) = hex_to_u128(hexv) {
                                    series.push(v);
                                }
                            }
                        }
                        reward_perc.push((*pct as u8, series));
                    }

                    // suggestions from last block percentiles
                    let last_idx = if block_count > 0 {
                        (block_count - 1) as usize
                    } else {
                        0
                    };
                    let pick_priority = |target: f64| -> Option<u128> {
                        let idx = FEE_HISTORY_PERCENTILES
                            .iter()
                            .position(|p| (*p - target).abs() < f64::EPSILON)?;
                        h.reward
                            .get(last_idx)
                            .and_then(|row| row.get(idx))
                            .and_then(|hexv| hex_to_u128(hexv))
                    };
                    let prio_safe = pick_priority(25.0)
                        .or_else(|| pick_priority(10.0))
                        .or_else(|| pick_priority(50.0));
                    let prio_std = pick_priority(50.0).or(prio_safe);
                    let prio_fast = pick_priority(75.0)
                        .or_else(|| pick_priority(90.0))
                        .or(prio_std);

                    let mk = |prio: Option<u128>| -> SuggestedFeeTier {
                        if let (Some(p), Some(next)) = (prio, self.metrics.next_base_fee_per_gas) {
                            let max_fee =
                                next.saturating_add(((p as f64) * SUGGESTION_RAMP_FACTOR) as u128);
                            SuggestedFeeTier {
                                max_fee_per_gas: max_fee,
                                max_priority_fee_per_gas: p,
                            }
                        } else {
                            SuggestedFeeTier::default()
                        }
                    };
                    self.metrics.suggested_fees = Some(SuggestedFees {
                        safe: mk(prio_safe),
                        standard: mk(prio_std),
                        fast: mk(prio_fast),
                    });

                    self.metrics.fee_history = Some(FeeHistoryMetrics {
                        oldest_block: hex_to_u64(&h.oldest_block).unwrap_or(0),
                        block_count,
                        base_fees,
                        gas_used_ratios,
                        reward_percentiles: reward_perc,
                    });
                }
                Err(_e) => {
                    // null out dependent fields, don't change status
                    self.metrics.fee_history = None;
                    self.metrics.base_fee_per_gas = None;
                    self.metrics.next_base_fee_per_gas = None;
                    self.metrics.suggested_fees = None;
                    self.metrics.gas_utilization_ma_n = None;
                    self.metrics.gas_volatility_5m = None;
                }
            }

            // RPC-suggested maxPriorityFeePerGas (best-effort)
            match self.client.provider.get_max_priority_fee_per_gas().await {
                Ok(p) => self.metrics.max_priority_fee_suggested = Some(p),
                Err(_) => self.metrics.max_priority_fee_suggested = None,
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

#[derive(Debug, Clone, Default)]
pub struct TxPoolTx {
    pub hash: B256,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub nonce: u128,
    pub gas_limit: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
    pub gas_price: Option<u128>,
    pub tx_type: Option<u8>,
}

impl TxPoolTx {
    fn from_wire(tx: &TxPoolTransactionWire) -> Option<Self> {
        let hash = parse_hex_b256(&tx.hash).unwrap_or_default();
        let from = parse_hex_address(&tx.from)?;
        let to = parse_hex_address(&tx.to);
        let value = parse_hex_u256(&tx.value).unwrap_or_else(|| U256::ZERO);
        let nonce = parse_hex_u128(&tx.nonce).unwrap_or(0);
        let gas_limit = parse_hex_u128(&tx.gas);
        let max_fee_per_gas = parse_hex_u128(&tx.max_fee_per_gas);
        let max_priority_fee_per_gas = parse_hex_u128(&tx.max_priority_fee_per_gas);
        let gas_price = parse_hex_u128(&tx.gas_price);
        let tx_type = parse_hex_u128(&tx.tx_type).map(|v| v as u8);

        Some(Self {
            hash,
            from,
            to,
            value,
            nonce,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            gas_price,
            tx_type,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TxPoolCursor {
    #[serde(rename = "txnHash")]
    pub txn_hash: Option<String>,
    pub score: Option<u64>,
    #[serde(rename = "globalTransactionScoreKey")]
    pub global_transaction_score_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxPoolTransactionsResponse {
    pub transactions: Option<Vec<TxPoolTransactionWire>>,
    pub cursor: Option<TxPoolCursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxPoolTransactionWire {
    #[serde(rename = "hash")]
    pub hash: Option<String>,
    #[serde(rename = "from")]
    pub from: Option<String>,
    #[serde(rename = "to")]
    pub to: Option<String>,
    #[serde(rename = "value")]
    pub value: Option<String>,
    #[serde(rename = "nonce")]
    pub nonce: Option<String>,
    #[serde(rename = "gas")]
    pub gas: Option<String>,
    #[serde(rename = "gasPrice")]
    pub gas_price: Option<String>,
    #[serde(rename = "maxFeePerGas")]
    pub max_fee_per_gas: Option<String>,
    #[serde(rename = "maxPriorityFeePerGas")]
    pub max_priority_fee_per_gas: Option<String>,
    #[serde(rename = "type")]
    pub tx_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TxPoolMetrics {
    pub healthy: bool,
    pub last_updated: Instant,
    pub error: Option<String>,
    pub base_url: String,
    pub transactions_cache: Option<u64>,
    pub bundles_cache: Option<u64>,
    pub signed_orders_cache: Option<u64>,
    pub transactions: VecDeque<TxPoolTx>,
    pub has_more: bool,
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
            transactions: VecDeque::new(),
            has_more: false,
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
            transactions: VecDeque::new(),
            has_more: false,
        }
    }
}

pub struct TxPoolClient {
    base_url: String,
    http: reqwest::Client,
    max_rows: usize,
    fetch_list: bool,
}

impl TxPoolClient {
    pub fn new(base_url: String, max_rows: usize, fetch_list: bool) -> Self {
        Self {
            base_url,
            http: reqwest::Client::new(),
            max_rows: max_rows.max(1),
            fetch_list,
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

    async fn fetch_transactions(&self, cursor: Option<&TxPoolCursor>) -> Result<(Vec<TxPoolTx>, bool)> {
        if !self.fetch_list {
            return Ok((Vec::new(), false));
        }

        let mut req = self.http.get(&self.join_url("/transactions"));
        if let Some(c) = cursor {
            if let Some(v) = &c.txn_hash {
                req = req.query(&[("txnHash", v)]);
            }
            if let Some(v) = c.score {
                req = req.query(&[("score", v)]);
            }
            if let Some(v) = &c.global_transaction_score_key {
                req = req.query(&[("globalTransactionScoreKey", v)]);
            }
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(eyre::eyre!(format!("/transactions HTTP {}", resp.status())));
        }

        let body = resp.text().await?;
        let parsed: TxPoolTransactionsResponse = serde_json::from_str(&body)?;
        let has_more = parsed.cursor.is_some();
        let mut out: Vec<TxPoolTx> = parsed
            .transactions
            .unwrap_or_default()
            .into_iter()
            .filter_map(|t| TxPoolTx::from_wire(&t))
            .collect();

        if out.len() > self.max_rows {
            out.truncate(self.max_rows);
        }
        Ok((out, has_more))
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

        match self.fetch_transactions(None).await {
            Ok((txs, has_more)) => {
                out.transactions = VecDeque::from(txs);
                out.has_more = has_more;
            }
            Err(e) => errors.push(format!("transactions list: {}", e)),
        }

        if errors.is_empty() {
            out.healthy = true;
        } else {
            out.healthy = out.transactions_cache.is_some()
                || out.bundles_cache.is_some()
                || out.signed_orders_cache.is_some()
                || !out.transactions.is_empty();
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

fn parse_hex_u128(s: &Option<String>) -> Option<u128> {
    let raw = s.as_ref()?;
    let trimmed = raw.trim_start_matches("0x");
    u128::from_str_radix(trimmed, 16).ok()
}

fn parse_hex_u256(s: &Option<String>) -> Option<U256> {
    let raw = s.as_ref()?;
    U256::from_str(raw).ok()
}

fn parse_hex_b256(s: &Option<String>) -> Option<B256> {
    let raw = s.as_ref()?;
    B256::from_str(raw).ok()
}

fn parse_hex_address(s: &Option<String>) -> Option<Address> {
    let raw = s.as_ref()?;
    Address::from_str(raw).ok()
}
