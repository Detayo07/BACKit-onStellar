use soroban_sdk::{contracttype, symbol_short, Address, Symbol};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    pub token_id: u64,
    pub recipient: Address,
    pub badge_type: Symbol,
    pub metadata_uri: soroban_sdk::String,
    pub minted_at: u64,
    /// 1 (lowest) to 5 (highest); legacy `award_badge` badges are tier 1.
    pub tier: u32,
}

/// Highest badge tier.
pub const MAX_TIER: u32 = 5;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ReputationConfig {
    pub admin: Address,
}

pub const BADGE_TOP10: Symbol = symbol_short!("TOP10");
pub const BADGE_100CORRECT: Symbol = symbol_short!("100COR");
pub const BADGE_EARLY: Symbol = symbol_short!("EARLY");
pub const BADGE_MILLION: Symbol = symbol_short!("MILLION");
