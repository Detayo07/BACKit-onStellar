#![no_std]

mod errors;
mod events;
pub mod performance;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use types::{MarketplaceConfig, OracleProvider, OracleRating};

use errors::OracleMarketplaceError;
use events::{
    emit_heartbeat, emit_oracle_deregistered, emit_oracle_rated, emit_oracle_registered,
    emit_oracle_selected, emit_resolution_recorded,
};
use performance::OraclePerformance;
use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Map, Vec};
use storage::*;

#[contract]
pub struct OracleMarketplace;

#[contractimpl]
impl OracleMarketplace {
    pub fn initialize(
        env: Env,
        admin: Address,
        cooldown_secs: u64,
        default_fee_bps: u32,
    ) -> Result<(), OracleMarketplaceError> {
        if get_config(&env).is_some() {
            return Err(OracleMarketplaceError::AlreadyInitialized);
        }
        admin.require_auth();

        let config = MarketplaceConfig {
            admin,
            cooldown_secs,
            default_fee_bps,
        };
        set_config(&env, &config);
        Ok(())
    }

    pub fn register_oracle(
        env: Env,
        provider: Address,
        pubkey: BytesN<32>,
        fee_bps: u32,
        min_stake: i128,
    ) -> Result<(), OracleMarketplaceError> {
        provider.require_auth();

        if get_oracle(&env, &pubkey).is_some() {
            return Err(OracleMarketplaceError::OracleAlreadyRegistered);
        }
        if fee_bps > 10000 {
            return Err(OracleMarketplaceError::InvalidFee);
        }

        let oracle = OracleProvider {
            pubkey: pubkey.clone(),
            address: provider.clone(),
            fee_bps,
            min_stake,
            staked_amount: min_stake,
            total_resolved: 0,
            total_disputes: 0,
            is_active: true,
            registered_at: env.ledger().timestamp(),
            deregister_after: None,
        };

        set_oracle(&env, &pubkey, &oracle);
        add_to_oracle_list(&env, &pubkey);
        performance::record_heartbeat(&env, &pubkey);
        emit_oracle_registered(&env, &provider, &pubkey, fee_bps);

        Ok(())
    }

    pub fn deregister_oracle(
        env: Env,
        provider: Address,
        pubkey: BytesN<32>,
    ) -> Result<(), OracleMarketplaceError> {
        provider.require_auth();
        let config = get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)?;
        let oracle = get_oracle(&env, &pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;

        if !oracle.is_active {
            return Err(OracleMarketplaceError::OracleNotActive);
        }

        if let Some(deregister_after) = oracle.deregister_after {
            if env.ledger().timestamp() < deregister_after {
                return Err(OracleMarketplaceError::CooldownActive);
            }
        }

        let mut updated = oracle;
        updated.is_active = false;
        updated.deregister_after = Some(env.ledger().timestamp() + config.cooldown_secs);
        set_oracle(&env, &pubkey, &updated);
        remove_from_oracle_list(&env, &pubkey);

        emit_oracle_deregistered(&env, &provider, &pubkey);
        Ok(())
    }

    pub fn get_available_oracles(env: Env) -> Vec<OracleProvider> {
        let list = get_oracle_list(&env);
        let mut result = Vec::new(&env);
        for i in 0..list.len() {
            let pubkey = list.get(i).unwrap();
            if let Some(oracle) = get_oracle(&env, &pubkey) {
                if oracle.is_active && performance::is_live(&env, &pubkey) {
                    result.push_back(oracle);
                }
            }
        }
        result
    }

    pub fn select_oracle_for_call(
        env: Env,
        call_id: u64,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(), OracleMarketplaceError> {
        let oracle = get_oracle(&env, &oracle_pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;
        if !oracle.is_active {
            return Err(OracleMarketplaceError::OracleNotActive);
        }
        if !performance::is_live(&env, &oracle_pubkey) {
            return Err(OracleMarketplaceError::OracleNotLive);
        }

        set_call_oracle(&env, call_id, &oracle_pubkey);
        emit_oracle_selected(&env, call_id, &oracle_pubkey);
        Ok(())
    }

    pub fn get_call_oracle(env: Env, call_id: u64) -> Option<BytesN<32>> {
        get_call_oracle(&env, call_id)
    }

    pub fn rate_oracle(
        env: Env,
        user: Address,
        oracle_pubkey: BytesN<32>,
        satisfied: bool,
    ) -> Result<(), OracleMarketplaceError> {
        user.require_auth();
        let _oracle = get_oracle(&env, &oracle_pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;

        let mut ratings = get_oracle_ratings(&env, &oracle_pubkey);
        if ratings.contains_key(user.clone()) {
            return Err(OracleMarketplaceError::AlreadyRated);
        }

        ratings.set(user.clone(), satisfied);
        set_oracle_ratings(&env, &oracle_pubkey, &ratings);

        emit_oracle_rated(&env, &oracle_pubkey, &user, satisfied);
        Ok(())
    }

    pub fn get_oracle_metrics(
        env: Env,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(u64, u64), OracleMarketplaceError> {
        let oracle = get_oracle(&env, &oracle_pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;
        Ok((oracle.total_resolved, oracle.total_disputes))
    }

    // ── Liveness (SC-030) ──────────────────────────────────────────────────

    /// Prove the oracle is alive. Must be sent at least every 17,280 ledgers
    /// (~24h) to stay eligible for new market assignments; sending one after a
    /// lapse restores eligibility immediately.
    pub fn submit_heartbeat(env: Env, provider: Address, pubkey: BytesN<32>) -> Result<(), OracleMarketplaceError> {
        provider.require_auth();
        let oracle = get_oracle(&env, &pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;
        if oracle.address != provider {
            return Err(OracleMarketplaceError::Unauthorized);
        }
        performance::record_heartbeat(&env, &pubkey);
        emit_heartbeat(&env, &pubkey, env.ledger().sequence());
        Ok(())
    }

    pub fn get_last_heartbeat(env: Env, pubkey: BytesN<32>) -> Option<u32> {
        performance::last_heartbeat(&env, &pubkey)
    }

    pub fn is_oracle_live(env: Env, pubkey: BytesN<32>) -> bool {
        performance::is_live(&env, &pubkey)
    }

    // ── Speed rewards (SC-029) ─────────────────────────────────────────────

    /// Choose the token bonuses are paid in (admin only).
    pub fn set_bonus_token(env: Env, token: Address) -> Result<(), OracleMarketplaceError> {
        get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)?.admin.require_auth();
        performance::set_bonus_token(&env, &token);
        Ok(())
    }

    /// Top up the bonus pool: tokens held by this contract that bonuses are paid from.
    pub fn fund_bonus_pool(env: Env, from: Address, amount: i128) -> Result<(), OracleMarketplaceError> {
        from.require_auth();
        if amount <= 0 {
            return Err(OracleMarketplaceError::InvalidAmount);
        }
        let token_addr = performance::bonus_token(&env).ok_or(OracleMarketplaceError::BonusTokenNotSet)?;
        token::Client::new(&env, &token_addr).transfer(&from, &env.current_contract_address(), &amount);
        Ok(())
    }

    /// Record a verified resolution and pay the speed bonus (admin only, so the
    /// timing and fee figures are attested by the protocol, not self-reported).
    ///
    /// `delta = submitted_at - market_expiry`: under 60s earns 1.5x, under 300s
    /// 1.2x, otherwise 1.0x. The bonus is `fee_amount * (multiplier - 1)`, paid to
    /// the oracle from the bonus pool. Returns the bonus paid.
    pub fn record_resolution(
        env: Env,
        oracle_pubkey: BytesN<32>,
        call_id: u64,
        market_expiry: u64,
        submitted_at: u64,
        fee_amount: i128,
    ) -> Result<i128, OracleMarketplaceError> {
        let config = get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)?;
        config.admin.require_auth();
        if fee_amount < 0 {
            return Err(OracleMarketplaceError::InvalidAmount);
        }
        let mut oracle = get_oracle(&env, &oracle_pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;
        if get_call_oracle(&env, call_id).as_ref() != Some(&oracle_pubkey) {
            return Err(OracleMarketplaceError::OracleNotSelectedForCall);
        }
        if performance::is_resolved(&env, call_id) {
            return Err(OracleMarketplaceError::AlreadyResolved);
        }

        let multiplier = performance::multiplier_bps(market_expiry, submitted_at);
        let bonus = fee_amount
            .checked_mul((multiplier - performance::BASE_MULTIPLIER_BPS) as i128)
            .ok_or(OracleMarketplaceError::InvalidAmount)?
            / performance::BASE_MULTIPLIER_BPS as i128;

        if bonus > 0 {
            let token_addr = performance::bonus_token(&env).ok_or(OracleMarketplaceError::BonusTokenNotSet)?;
            let asset = token::Client::new(&env, &token_addr);
            if asset.balance(&env.current_contract_address()) < bonus {
                return Err(OracleMarketplaceError::InsufficientBonusPool);
            }
            asset.transfer(&env.current_contract_address(), &oracle.address, &bonus);
        }

        let mut perf = performance::get_performance(&env, &oracle_pubkey);
        perf.resolutions += 1;
        if bonus > 0 {
            perf.bonus_resolutions += 1;
        }
        perf.bonus_paid += bonus;
        perf.score += multiplier as u64;
        performance::set_performance(&env, &oracle_pubkey, &perf);

        oracle.total_resolved += 1;
        set_oracle(&env, &oracle_pubkey, &oracle);
        performance::mark_resolved(&env, call_id);

        emit_resolution_recorded(&env, call_id, &oracle_pubkey, multiplier, bonus);
        Ok(bonus)
    }

    pub fn get_oracle_performance(env: Env, oracle_pubkey: BytesN<32>) -> OraclePerformance {
        performance::get_performance(&env, &oracle_pubkey)
    }

    pub fn get_config_view(env: Env) -> Result<MarketplaceConfig, OracleMarketplaceError> {
        get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)
    }
}
