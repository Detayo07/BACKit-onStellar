#![allow(deprecated)]

use soroban_sdk::{Address, Env, Symbol};

pub fn emit_badge_revoked(env: &Env, user: &Address, badge_type: &Symbol, token_id: u64) {
    env.events().publish(
        ("reputation_nft", "BadgeRevoked"),
        (user.clone(), badge_type.clone(), token_id),
    );
}

pub fn emit_badge_awarded(env: &Env, user: &Address, badge_type: &Symbol, token_id: u64) {
    env.events().publish(
        ("reputation_nft", "BadgeAwarded"),
        (user.clone(), badge_type.clone(), token_id),
    );
}
