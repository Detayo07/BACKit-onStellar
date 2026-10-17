use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum ReputationError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    BadgeAlreadyAwarded = 4,
    TransferNotAllowed = 5,
    InvalidBadgeType = 6,
    /// Badges are soulbound: `transfer` and `transfer_from` always fail with this.
    NonTransferable = 7,
    BadgeNotFound = 8,
    /// Tier must be between 1 and 5.
    InvalidTier = 9,
}
