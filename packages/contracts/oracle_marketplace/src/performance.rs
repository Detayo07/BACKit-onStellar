//! Oracle speed rewards (SC-029) and liveness (SC-030).
use soroban_sdk::{contracttype, BytesN, Env};

/// Resolutions within this many seconds of market expiry earn the top multiplier.
pub const FAST_WINDOW_SECS: u64 = 60;
/// Resolutions within this many seconds earn the middle multiplier.
pub const MEDIUM_WINDOW_SECS: u64 = 300;
/// 1.5x, 1.2x and 1.0x, in basis points.
pub const FAST_MULTIPLIER_BPS: u32 = 15_000;
pub const MEDIUM_MULTIPLIER_BPS: u32 = 12_000;
pub const BASE_MULTIPLIER_BPS: u32 = 10_000;
/// An oracle idle for more than this many ledgers (~24 hours) is not routed new markets.
pub const HEARTBEAT_MAX_AGE_LEDGERS: u32 = 17_280;

/// Reward multiplier for resolving `submitted_at` seconds relative to `expiry`.
/// Submitting before expiry counts as zero latency.
pub fn multiplier_bps(expiry: u64, submitted_at: u64) -> u32 {
    let delta = submitted_at.saturating_sub(expiry);
    if delta < FAST_WINDOW_SECS {
        FAST_MULTIPLIER_BPS
    } else if delta < MEDIUM_WINDOW_SECS {
        MEDIUM_MULTIPLIER_BPS
    } else {
        BASE_MULTIPLIER_BPS
    }
}

/// Running performance record for one oracle.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OraclePerformance {
    pub resolutions: u64,
    /// Resolutions that earned a bonus (multiplier above 1.0x).
    pub bonus_resolutions: u64,
    pub bonus_paid: i128,
    /// Sum of the multiplier (basis points) of every resolution: 10_000 per
    /// ordinary resolution, up to 15_000 for a fast one.
    pub score: u64,
}

#[contracttype]
enum PerfKey {
    Performance(BytesN<32>),
    Heartbeat(BytesN<32>),
    Resolved(u64),
    BonusToken,
}

pub fn get_performance(env: &Env, pubkey: &BytesN<32>) -> OraclePerformance {
    env.storage()
        .persistent()
        .get(&PerfKey::Performance(pubkey.clone()))
        .unwrap_or(OraclePerformance {
            resolutions: 0,
            bonus_resolutions: 0,
            bonus_paid: 0,
            score: 0,
        })
}

pub fn set_performance(env: &Env, pubkey: &BytesN<32>, perf: &OraclePerformance) {
    env.storage()
        .persistent()
        .set(&PerfKey::Performance(pubkey.clone()), perf);
}

pub fn is_resolved(env: &Env, call_id: u64) -> bool {
    env.storage().persistent().has(&PerfKey::Resolved(call_id))
}

pub fn mark_resolved(env: &Env, call_id: u64) {
    env.storage().persistent().set(&PerfKey::Resolved(call_id), &true);
}

pub fn bonus_token(env: &Env) -> Option<soroban_sdk::Address> {
    env.storage().instance().get(&PerfKey::BonusToken)
}

pub fn set_bonus_token(env: &Env, token: &soroban_sdk::Address) {
    env.storage().instance().set(&PerfKey::BonusToken, token);
}

pub fn record_heartbeat(env: &Env, pubkey: &BytesN<32>) {
    let key = PerfKey::Heartbeat(pubkey.clone());
    env.storage().persistent().set(&key, &env.ledger().sequence());
    // Keep the record alive well past the liveness window; if it were archived
    // the oracle would look like it never sent a heartbeat.
    env.storage().persistent().extend_ttl(
        &key,
        HEARTBEAT_MAX_AGE_LEDGERS * 2,
        HEARTBEAT_MAX_AGE_LEDGERS * 3,
    );
}

pub fn last_heartbeat(env: &Env, pubkey: &BytesN<32>) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&PerfKey::Heartbeat(pubkey.clone()))
}

/// An oracle is live if it heartbeat within [`HEARTBEAT_MAX_AGE_LEDGERS`]. An
/// oracle with no heartbeat on record (registered before liveness tracking) is
/// treated as live until its first heartbeat window can be established.
pub fn is_live(env: &Env, pubkey: &BytesN<32>) -> bool {
    match last_heartbeat(env, pubkey) {
        Some(last) => env.ledger().sequence().saturating_sub(last) <= HEARTBEAT_MAX_AGE_LEDGERS,
        None => true,
    }
}
