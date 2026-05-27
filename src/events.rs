//! Signet system-event classification and log-to-event mapping.
//!
//! Event signatures are best-effort: if a deployed contract uses a
//! different parameter list the log will still be captured (we filter
//! by contract address) but will appear with a truncated topic-0 hash
//! instead of a human-readable name.

use alloy::primitives::{Address, B256};
use alloy::rpc::types::Log;
use alloy::sol_types::SolEvent;
use signet_constants::SignetSystemConstants;
use std::collections::HashMap;
use std::time::Instant;

// ========================= EVENT SIGNATURES =========================

alloy::sol! {
    // Zenith (host)
    event BlockSubmitted(uint256 indexed rollupChainId, address indexed builder, uint256 indexed blockHeight);
    event SequencerSet(uint256 indexed rollupChainId, address indexed sequencer);

    // Orders (host + rollup)
    event Order(bytes32 indexed orderId);
    event Filled(bytes32 indexed orderId, address indexed filler);
    event Sweep(address indexed recipient, address indexed token, uint256 amount);

    // Passage – host side
    event Enter(address indexed rollupRecipient, uint256 amount);
    event EnterToken(address indexed rollupRecipient, address indexed token, uint256 amount);
    event EnterConfigured(uint256 gasLimit, address target);

    // Passage – rollup side
    event Exit(address indexed hostRecipient, uint256 amount);
    event ExitToken(address indexed hostRecipient, address indexed token, uint256 amount);
    event Withdrawal(address indexed hostRecipient, uint256 amount);

    // Transactor (host)
    event Transact(address indexed sender, address indexed target, uint256 value, uint256 gas, bytes data);
    event GasConfigured(uint256 gasLimit, uint256 gasPrice);
}

// ========================= EVENT MODEL =========================

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SystemEvent {
    pub block_number: u64,
    pub tx_hash: B256,
    pub log_index: u64,
    pub chain_label: &'static str,
    pub contract_label: &'static str,
    pub event_name: String,
    pub summary: String,
    pub timestamp: Option<u64>,
    pub observed_at: Instant,
}

// ========================= REGISTRY =========================

pub struct EventRegistry {
    signatures: HashMap<B256, &'static str>,
    contracts: HashMap<Address, &'static str>,
    chain_label: &'static str,
}

impl EventRegistry {
    /// Build a registry for the given chain, or `None` if the chain
    /// has no known Signet system contracts.
    pub fn for_chain(chain_id: u64) -> Option<Self> {
        let (chain_label, contracts) = event_contracts_for_chain_id(chain_id);
        if contracts.is_empty() {
            return None;
        }
        Some(Self::new(contracts, chain_label))
    }

    fn new(contracts: Vec<(Address, &'static str)>, chain_label: &'static str) -> Self {
        let signatures: HashMap<B256, &'static str> = [
            (BlockSubmitted::SIGNATURE_HASH, "BlockSubmitted"),
            (SequencerSet::SIGNATURE_HASH, "SequencerSet"),
            (Order::SIGNATURE_HASH, "Order"),
            (Filled::SIGNATURE_HASH, "Filled"),
            (Sweep::SIGNATURE_HASH, "Sweep"),
            (Enter::SIGNATURE_HASH, "Enter"),
            (EnterToken::SIGNATURE_HASH, "EnterToken"),
            (EnterConfigured::SIGNATURE_HASH, "EnterConfigured"),
            (Exit::SIGNATURE_HASH, "Exit"),
            (ExitToken::SIGNATURE_HASH, "ExitToken"),
            (Withdrawal::SIGNATURE_HASH, "Withdrawal"),
            (Transact::SIGNATURE_HASH, "Transact"),
            (GasConfigured::SIGNATURE_HASH, "GasConfigured"),
        ]
        .into_iter()
        .collect();

        let contract_map: HashMap<Address, &'static str> = contracts.into_iter().collect();

        Self {
            signatures,
            contracts: contract_map,
            chain_label,
        }
    }

    /// Try to turn a raw RPC log into a classified [`SystemEvent`].
    pub fn classify_log(&self, log: &Log) -> Option<SystemEvent> {
        let block_number = log.block_number?;
        let tx_hash = log.transaction_hash?;
        let log_index = log.log_index?;
        let topics = log.data().topics();

        let topic0 = topics.first()?;

        let event_name = self
            .signatures
            .get(topic0)
            .map(|n| n.to_string())
            .unwrap_or_else(|| {
                let h = format!("{:#x}", topic0);
                if h.len() > 10 {
                    format!("{}…", &h[..10])
                } else {
                    h
                }
            });

        let contract_label = self.contracts.get(&log.address()).copied().unwrap_or("?");

        let summary = build_summary(&event_name, topics);

        Some(SystemEvent {
            block_number,
            tx_hash,
            log_index,
            chain_label: self.chain_label,
            contract_label,
            event_name,
            summary,
            timestamp: log.block_timestamp,
            observed_at: Instant::now(),
        })
    }

    pub fn filter_addresses(&self) -> Vec<Address> {
        self.contracts.keys().copied().collect()
    }
}

// ========================= HELPERS =========================

fn build_summary(event_name: &str, topics: &[B256]) -> String {
    match event_name {
        "BlockSubmitted" => topics
            .get(3)
            .map(|t| {
                let n = u64::from_be_bytes(t.0[24..32].try_into().unwrap_or_default());
                format!("#{}", n)
            })
            .unwrap_or_default(),
        "SequencerSet" => topics.get(2).map(short_topic_addr).unwrap_or_default(),
        "Enter" | "Exit" | "Withdrawal" => topics.get(1).map(short_topic_addr).unwrap_or_default(),
        "EnterToken" | "ExitToken" => {
            let recipient = topics.get(1).map(short_topic_addr).unwrap_or_default();
            let token = topics.get(2).map(short_topic_addr).unwrap_or_default();
            format!("{} tkn:{}", recipient, token)
        }
        "Order" | "Filled" => topics
            .get(1)
            .map(|t| {
                let h = format!("{:#x}", t);
                if h.len() > 14 {
                    format!("{}..{}", &h[..8], &h[h.len() - 4..])
                } else {
                    h
                }
            })
            .unwrap_or_default(),
        "Sweep" => topics.get(1).map(short_topic_addr).unwrap_or_default(),
        "Transact" => {
            let sender = topics.get(1).map(short_topic_addr).unwrap_or_default();
            let target = topics.get(2).map(short_topic_addr).unwrap_or_default();
            format!("{} -> {}", sender, target)
        }
        _ => topics
            .get(1)
            .map(|t| {
                let h = format!("{:#x}", t);
                if h.len() > 14 {
                    format!("{}..{}", &h[..8], &h[h.len() - 4..])
                } else {
                    h
                }
            })
            .unwrap_or_default(),
    }
}

/// Render an address-in-topic as a short hex string.
fn short_topic_addr(topic: &B256) -> String {
    let addr = Address::from_slice(&topic[12..]);
    let s = format!("{:#x}", addr);
    if s.len() > 14 {
        format!("{}..{}", &s[..8], &s[s.len() - 4..])
    } else {
        s
    }
}

/// Map a chain ID to the Signet system contracts to watch and a
/// human-readable chain label.
fn event_contracts_for_chain_id(chain_id: u64) -> (&'static str, Vec<(Address, &'static str)>) {
    for constants in [
        SignetSystemConstants::parmigiana(),
        SignetSystemConstants::gouda(),
        SignetSystemConstants::mainnet(),
    ] {
        if constants.host_chain_id() == chain_id {
            return (
                "Host",
                vec![
                    (constants.host_zenith(), "Zenith"),
                    (constants.host_orders(), "Orders"),
                    (constants.host_passage(), "Passage"),
                    (constants.host_transactor(), "Transactor"),
                ],
            );
        }
        if constants.ru_chain_id() == chain_id {
            return (
                "Rollup",
                vec![
                    (constants.ru_orders(), "Orders"),
                    (constants.ru_passage(), "Passage"),
                ],
            );
        }
    }
    ("Unknown", vec![])
}
