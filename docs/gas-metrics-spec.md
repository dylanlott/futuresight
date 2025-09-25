# Gas Tracking Spec for SignetMetrics

This spec adds first-class gas tracking to the dashboard. It introduces new fields on `SignetMetrics` and `BlockInfo`, a collection strategy leveraging `eth_feeHistory` and block headers, and UI updates to present actionable fee suggestions and network utilization signals.

## Goals

- Provide accurate, timely gas insights for EIP-1559 and EIP-4844 networks
- Surface practical fee suggestions (safe/standard/fast) for users and bots
- Visualize block gas utilization and detect spikes/outliers
- Remain robust across chains that lack certain EIPs (graceful degradation)

## Scope at a Glance

- New headline gas fields: base fee, next base fee, legacy gas price, suggested priority fee
- Fee history percentiles for tips and utilization moving averages
- EIP-4844 blob/data gas metrics when available
- Alerts for high gas and sudden spikes
- Tunable percentiles and history window

## Data Model Changes

All monetary values are stored in wei in the data model and formatted as gwei for display.

### SignetMetrics (new fields)

- base_fee_per_gas: Option<u128>
- next_base_fee_per_gas: Option<u128>  // predicted next base fee from fee history
- max_priority_fee_suggested: Option<u128>  // from eth_maxPriorityFeePerGas
- suggested_fees: Option<SuggestedFees>
- fee_history: Option<FeeHistoryMetrics>
- gas_utilization_ma_n: Option<f64>  // 0..=100, moving average over last N blocks
- gas_volatility_5m: Option<f64>     // pct change of base fee vs 5-min MA (or last 30 blocks)
- // EIP-4844 blob/data gas
- blob_base_fee: Option<u128>        // base fee per blob gas (if derivable/available)
- blob_base_fee_next: Option<u128>
- blob_gas_utilization_ma_n: Option<f64>  // 0..=100

Notes:
- Keep existing `gas_price` for legacy/fallback and quick glance.
- MA windows and percent thresholds are configurable (see Configs).

### BlockInfo (new fields)

- base_fee_per_gas: Option<u128>
- blob_gas_used: Option<u64>
- excess_blob_gas: Option<u64>

These mirror JSON-RPC block header fields when present. If absent on a chain, fields remain None.

### Helper structs

- struct SuggestedFeeTier { max_fee_per_gas: u128, max_priority_fee_per_gas: u128 }
- struct SuggestedFees { safe: SuggestedFeeTier, standard: SuggestedFeeTier, fast: SuggestedFeeTier }
- struct FeeHistoryMetrics {
  - oldest_block: u64
  - block_count: u64
  - base_fees: Vec<u128>                 // length = block_count
  - gas_used_ratios: Vec<f64>            // 0..=100 per block
  - reward_percentiles: Vec<(u8, Vec<u128>)> // [(percentile, rewards per block)]
}

## Collection Strategy

Poll cadence remains 5s (existing). Gas collection happens when RPC status is Connected.

### Calls and computations per cycle

1) Headline values
- eth_gasPrice -> gas_price (legacy/fallback)
- eth_maxPriorityFeePerGas -> max_priority_fee_suggested

2) Fee history (1559) for percentiles and predictions
- eth_feeHistory(blockCount=N, newest=latest, rewardPercentiles=[10,25,50,75,90])
  - Capture:
    - baseFeePerGas array (N values for included blocks). Also includes the next base fee; store that as next_base_fee_per_gas
    - gasUsedRatio per block -> convert to 0..=100 percentage
    - reward arrays per percentile -> store under `reward_percentiles`
  - Compute:
    - gas_utilization_ma_n = average(gasUsedRatio%) over last N
    - gas_volatility_5m = pct change of the latest base fee vs average over ~30 blocks (or 5 minutes worth if block time is known/configured)

3) Suggested fees (1559)
- Define tiers using percentiles and next base fee:
  - priority candidates: p25 (safe), p50 (standard), p75 or p90 (fast)
  - next base fee comes from fee history response (the extra last value)
  - compute maxFeePerGas using a conservative ramp factor to cover short-term rises:
    - maxFeePerGas = nextBaseFee + 2 * priorityFee (default)
  - Emit SuggestedFees { safe, standard, fast }
- If 1559 unsupported, omit SuggestedFees and rely on gas_price only

4) Block headers (already fetched)
- Record per-block base_fee_per_gas into BlockInfo when present
- Derive blob/data gas when present:
  - capture blob_gas_used and excess_blob_gas from headers
  - compute blob_base_fee and blob_base_fee_next if available via provider or derived; if not readily available, leave None
- Compute `blob_gas_utilization_ma_n` analogous to gas utilization using per-block blob gas used vs target where available; if target not readily available, treat as ratio to protocol target (implementation detail) or omit MA until target is exposed

### Backfill and limits

- Reuse existing backfill strategy with MAX_BACKFILL_PER_CYCLE to populate BlockInfo fields gradually
- Fee history window N is configurable (default 20)

### Failure handling

- Any failing call sets only the impacted fields to None, does not flip overall connection status
- Retry on next cycle; maintain last known good values for display where appropriate

## UI Integration

Update the Gas section to include:

- Base fee (gwei), Priority p50 (gwei), Legacy gas price (gwei + wei in parens)
- Suggested tiers:
  - Safe: maxFee/prio in gwei
  - Std: maxFee/prio in gwei
  - Fast: maxFee/prio in gwei
- Utilization:
  - Last block gas utilization and MA(N)
- If EIP-4844 present:
  - Blob base fee (gwei), blob utilization MA(N)

Recent Blocks area remains compact but can optionally append base fee short value for the newest block (e.g., "bf:18g") if space permits.

Colors and emphasis:
- High base fee or spikes render in yellow/red based on configured thresholds
- Blob congestion similarly highlighted

## Alerts

- High gas: base_fee_per_gas or p50 priority above a configured gwei threshold -> WARN/ALERT
- Spike detector: latest base fee > 2x 5-min MA -> ALERT
- 4844 congestion: blob_base_fee above threshold -> WARN

Alerts do not affect connection status; they render in their own box similar to block delay.

## Configuration Additions

Extend `Config` with optional parameters (sensible defaults shown):

- fee_history_blocks: u64 = 20
- fee_history_percentiles: Vec<u8> = [10,25,50,75,90]
- suggestion_ramp_factor: f64 = 2.0 // used in maxFee computation
- gas_alert_high_gwei: f64 = 100.0
- gas_spike_multiplier: f64 = 2.0
- blob_alert_high_gwei: f64 = 5.0
- utilization_ma_window: usize = 20

## Edge Cases and Compatibility

- Pre-1559 chains: base fee/fee history unavailable
  - Show only legacy gas_price and omit SuggestedFees; utilization still computed per block from gas_used/gas_limit
- Pre-4844 or non-blob networks: blob_* fields remain None and UI hides them
- L2s and rollups: fee history semantics may differ; rely on available fields and degrade gracefully
- Missing/partial fee history: proceed with the subset received; skip suggestions if next base fee missing

## Performance Considerations

- One `eth_feeHistory` call per poll with small N is efficient and low overhead
- Avoid per-tx scanning; we do not pull mempool tips unless a separate endpoint provides aggregates
- Keep BlockInfo enriched lazily during backfill to avoid large bursts

## Acceptance Criteria

- Headline fields populate on mainnet (or any 1559 chain) within a single poll
- SuggestedFees present with non-zero values in gwei on supported networks
- UI shows base fee, p50 priority, legacy price, and three suggestion tiers
- Utilization MA displays and updates smoothly as new blocks arrive
- Alerts trigger when configured thresholds are exceeded (can be simulated by lowering thresholds)
- No panics on unsupported chains; fields read as "N/A" and UI adapts

## Nice-to-haves / Future Work

- Optional integration with tx-pool-webservice for fee histograms if endpoints are added (e.g., /fee-histogram)
- Graph sparkline of base fee and priority percentiles over last N blocks
- Auto-adjust suggestion_ramp_factor based on recent base fee volatility
- Display EIP-4844 blob fee tiers once standard RPC exposes blob fee directly

## Implementation Notes (Rust + Alloy)

- Use provider.get_gas_price(), provider.get_chain_id() as today
- Implement fee history via JSON-RPC `eth_feeHistory` (if Alloy lacks a helper, call through the provider's raw interface)
- Extend `get_block_by_number` to map header fields to new BlockInfo attributes
- Keep units in wei internally; add small helpers for gwei formatting in the UI layer
- Respect `MAX_BACKFILL_PER_CYCLE` to cap history enrichment work per poll

---

This spec is intended to be incrementally implementable: start with base fee + fee history percentiles + suggestions; then add blob metrics where headers expose them.
