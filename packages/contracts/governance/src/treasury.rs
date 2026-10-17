//! Multi-bucket fee split engine (SC-032).
//!
//! Incoming protocol fees are split atomically into four buckets: protocol
//! treasury, creator incentives, token buyback, and SAC burn. The split is a
//! basis-point table that must sum to 10 000 and can only be changed with
//! authorisation from a threshold of the treasury signers.
use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::errors::GovernanceError;

pub const BPS_DENOM: i128 = 10_000;

/// Default split: 50% treasury, 25% creator incentives, 15% buyback, 10% burn.
pub const DEFAULT_SPLIT_BPS: [u32; 4] = [5_000, 2_500, 1_500, 1_000];

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bucket {
    Treasury = 0,
    CreatorIncentives = 1,
    Buyback = 2,
    Burn = 3,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct TreasuryConfig {
    pub signers: Vec<Address>,
    pub threshold: u32,
    pub split_bps: Vec<u32>,
}

#[contracttype]
enum TreasuryKey {
    TreasurySettings,
    Balance(Address, Bucket),
    Burned(Address),
}

pub fn get_config(env: &Env) -> Option<TreasuryConfig> {
    env.storage().instance().get(&TreasuryKey::TreasurySettings)
}

pub fn set_config(env: &Env, config: &TreasuryConfig) {
    env.storage().instance().set(&TreasuryKey::TreasurySettings, config);
}

pub fn default_split(env: &Env) -> Vec<u32> {
    Vec::from_array(env, DEFAULT_SPLIT_BPS)
}

pub fn validate_split(bps: &Vec<u32>) -> Result<(), GovernanceError> {
    if bps.len() != 4 {
        return Err(GovernanceError::InvalidSplit);
    }
    let mut sum: u32 = 0;
    for b in bps.iter() {
        sum = sum.checked_add(b).ok_or(GovernanceError::InvalidSplit)?;
    }
    if sum as i128 != BPS_DENOM {
        return Err(GovernanceError::InvalidSplit);
    }
    Ok(())
}

/// Split `amount` by `bps`. Each non-treasury bucket is rounded down and the
/// treasury bucket takes the remainder, so the parts always sum to `amount`.
pub fn split_amount(amount: i128, bps: &Vec<u32>) -> Result<[i128; 4], GovernanceError> {
    let part = |i: u32| -> Result<i128, GovernanceError> {
        amount
            .checked_mul(bps.get(i).unwrap_or(0) as i128)
            .map(|v| v / BPS_DENOM)
            .ok_or(GovernanceError::InvalidAmount)
    };
    let creators = part(1)?;
    let buyback = part(2)?;
    let burn = part(3)?;
    Ok([amount - creators - buyback - burn, creators, buyback, burn])
}

pub fn balance(env: &Env, token: &Address, bucket: Bucket) -> i128 {
    env.storage()
        .persistent()
        .get(&TreasuryKey::Balance(token.clone(), bucket))
        .unwrap_or(0)
}

pub fn set_balance(env: &Env, token: &Address, bucket: Bucket, amount: i128) {
    env.storage()
        .persistent()
        .set(&TreasuryKey::Balance(token.clone(), bucket), &amount);
}

pub fn total_burned(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&TreasuryKey::Burned(token.clone()))
        .unwrap_or(0)
}

pub fn add_burned(env: &Env, token: &Address, amount: i128) {
    env.storage()
        .persistent()
        .set(&TreasuryKey::Burned(token.clone()), &(total_burned(env, token) + amount));
}

/// Require that at least `threshold` distinct treasury signers appear in
/// `approvers` and have authorised the call.
pub fn require_signers(
    env: &Env,
    config: &TreasuryConfig,
    approvers: &Vec<Address>,
) -> Result<(), GovernanceError> {
    let mut seen: Vec<Address> = Vec::new(env);
    for a in approvers.iter() {
        if !config.signers.contains(&a) {
            return Err(GovernanceError::Unauthorized);
        }
        if !seen.contains(&a) {
            seen.push_back(a);
        }
    }
    if seen.len() < config.threshold {
        return Err(GovernanceError::NotEnoughSigners);
    }
    for a in seen.iter() {
        a.require_auth();
    }
    Ok(())
}
