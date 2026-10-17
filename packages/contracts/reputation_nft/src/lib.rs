#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use types::{Badge, ReputationConfig};

use errors::ReputationError;
use events::{emit_badge_awarded, emit_badge_revoked};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

#[contract]
pub struct ReputationNft;

#[contractimpl]
impl ReputationNft {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ReputationError> {
        if storage::get_config(&env).is_some() {
            return Err(ReputationError::AlreadyInitialized);
        }
        admin.require_auth();

        let config = types::ReputationConfig { admin };
        storage::set_config(&env, &config);
        Ok(())
    }

    pub fn award_badge(
        env: Env,
        recipient: Address,
        badge_type: Symbol,
        metadata_uri: soroban_sdk::String,
    ) -> Result<u64, ReputationError> {
        let config = storage::get_config(&env).ok_or(ReputationError::NotInitialized)?;
        config.admin.require_auth();

        if storage::has_badge_type(&env, &badge_type, &recipient) {
            return Err(ReputationError::BadgeAlreadyAwarded);
        }

        let token_id = storage::next_badge_id(&env);
        let badge = Badge {
            token_id,
            recipient: recipient.clone(),
            badge_type: badge_type.clone(),
            metadata_uri,
            minted_at: env.ledger().timestamp(),
            tier: 1,
        };

        storage::set_badge(&env, token_id, &badge);
        storage::add_user_badge(&env, &recipient, token_id);
        storage::mark_badge_awarded(&env, &badge_type, &recipient);

        emit_badge_awarded(&env, &recipient, &badge_type, token_id);

        Ok(token_id)
    }

    /// Mint a soulbound badge (admin only). `badge_id` must be `PARLAY_CHAMPION`,
    /// `TOP_TIER_ACCURACY` or one of the legacy badge types; `tier` is 1 to 5.
    /// A recipient can hold each badge once. Returns the new token id.
    pub fn mint(
        env: Env,
        recipient: Address,
        badge_id: Symbol,
        tier: u32,
        metadata_uri: soroban_sdk::String,
    ) -> Result<u64, ReputationError> {
        let config = storage::get_config(&env).ok_or(ReputationError::NotInitialized)?;
        config.admin.require_auth();

        if !Self::is_valid_badge(&env, &badge_id) {
            return Err(ReputationError::InvalidBadgeType);
        }
        if tier == 0 || tier > types::MAX_TIER {
            return Err(ReputationError::InvalidTier);
        }
        if storage::has_badge_type(&env, &badge_id, &recipient) {
            return Err(ReputationError::BadgeAlreadyAwarded);
        }

        let token_id = storage::next_badge_id(&env);
        storage::set_badge(
            &env,
            token_id,
            &Badge {
                token_id,
                recipient: recipient.clone(),
                badge_type: badge_id.clone(),
                metadata_uri,
                minted_at: env.ledger().timestamp(),
                tier,
            },
        );
        storage::add_user_badge(&env, &recipient, token_id);
        storage::mark_badge_awarded(&env, &badge_id, &recipient);
        emit_badge_awarded(&env, &recipient, &badge_id, token_id);
        Ok(token_id)
    }

    /// Revoke a badge (admin only): it is deleted and its holder may be awarded it again.
    pub fn revoke(env: Env, token_id: u64) -> Result<(), ReputationError> {
        let config = storage::get_config(&env).ok_or(ReputationError::NotInitialized)?;
        config.admin.require_auth();
        let badge = storage::get_badge(&env, token_id).ok_or(ReputationError::BadgeNotFound)?;

        storage::remove_badge(&env, token_id);
        storage::remove_user_badge(&env, &badge.recipient, token_id);
        storage::unmark_badge_awarded(&env, &badge.badge_type, &badge.recipient);
        emit_badge_revoked(&env, &badge.recipient, &badge.badge_type, token_id);
        Ok(())
    }

    /// Number of badges `owner` holds.
    pub fn balance(env: Env, owner: Address) -> u32 {
        storage::get_user_badge_ids(&env, &owner).len()
    }

    pub fn owner_of(env: Env, token_id: u64) -> Result<Address, ReputationError> {
        storage::get_badge(&env, token_id)
            .map(|b| b.recipient)
            .ok_or(ReputationError::BadgeNotFound)
    }

    /// Public metadata for a badge: its URI and tier.
    pub fn metadata(env: Env, token_id: u64) -> Result<(soroban_sdk::String, u32), ReputationError> {
        storage::get_badge(&env, token_id)
            .map(|b| (b.metadata_uri, b.tier))
            .ok_or(ReputationError::BadgeNotFound)
    }

    pub fn get_badge(env: Env, token_id: u64) -> Result<Badge, ReputationError> {
        storage::get_badge(&env, token_id).ok_or(ReputationError::BadgeNotFound)
    }

    /// Always fails: badges are soulbound.
    pub fn transfer_from(
        _env: Env,
        _spender: Address,
        _from: Address,
        _to: Address,
        _token_id: u64,
    ) -> Result<(), ReputationError> {
        Err(ReputationError::NonTransferable)
    }

    fn is_valid_badge(env: &Env, badge: &Symbol) -> bool {
        *badge == Symbol::new(env, "PARLAY_CHAMPION")
            || *badge == Symbol::new(env, "TOP_TIER_ACCURACY")
            || *badge == types::BADGE_TOP10
            || *badge == types::BADGE_100CORRECT
            || *badge == types::BADGE_EARLY
            || *badge == types::BADGE_MILLION
    }

    pub fn check_and_award_badges(env: Env, user: Address) -> Vec<u64> {
        let mut awarded = Vec::new(&env);
        let stats = Self::get_user_stats(&env, &user);

        if stats.correct_predictions >= 100
            && !storage::has_badge_type(&env, &types::BADGE_100CORRECT, &user)
        {
            if let Ok(id) = Self::award_badge(
                env.clone(),
                user.clone(),
                types::BADGE_100CORRECT,
                soroban_sdk::String::from_str(&env, "ipfs://100correct"),
            ) {
                awarded.push_back(id);
            }
        }

        if stats.total_predictions > 0
            && stats.accuracy_bps >= 8000
            && !storage::has_badge_type(&env, &types::BADGE_TOP10, &user)
        {
            if let Ok(id) = Self::award_badge(
                env.clone(),
                user.clone(),
                types::BADGE_TOP10,
                soroban_sdk::String::from_str(&env, "ipfs://top10"),
            ) {
                awarded.push_back(id);
            }
        }

        if stats.total_payout >= 1_000_000
            && !storage::has_badge_type(&env, &types::BADGE_MILLION, &user)
        {
            if let Ok(id) = Self::award_badge(
                env.clone(),
                user.clone(),
                types::BADGE_MILLION,
                soroban_sdk::String::from_str(&env, "ipfs://millionaire"),
            ) {
                awarded.push_back(id);
            }
        }

        awarded
    }

    pub fn get_user_badges(env: Env, user: Address) -> Vec<Badge> {
        let ids = storage::get_user_badge_ids(&env, &user);
        let mut result = Vec::new(&env);
        for i in 0..ids.len() {
            if let Some(badge) = storage::get_badge(&env, ids.get(i).unwrap()) {
                result.push_back(badge);
            }
        }
        result
    }

    pub fn transfer(
        _env: Env,
        _from: Address,
        _to: Address,
        _token_id: u64,
    ) -> Result<(), ReputationError> {
        Err(ReputationError::NonTransferable)
    }

    fn get_user_stats(_env: &Env, _user: &Address) -> UserStats {
        UserStats {
            total_predictions: 0,
            correct_predictions: 0,
            accuracy_bps: 0,
            total_payout: 0,
        }
    }

    pub fn get_config_view(env: Env) -> Result<ReputationConfig, ReputationError> {
        storage::get_config(&env).ok_or(ReputationError::NotInitialized)
    }
}

struct UserStats {
    total_predictions: u64,
    correct_predictions: u64,
    accuracy_bps: u32,
    total_payout: i128,
}
