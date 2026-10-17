use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum OracleMarketplaceError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    OracleAlreadyRegistered = 4,
    OracleNotFound = 5,
    OracleNotActive = 6,
    InsufficientStake = 7,
    CooldownActive = 8,
    InvalidFee = 9,
    CallNotFound = 10,
    OracleNotSelectedForCall = 11,
    AlreadyRated = 12,
    InvalidRating = 13,
    /// The oracle has not sent a heartbeat recently and is not routed new markets.
    OracleNotLive = 14,
    /// A resolution was already recorded for this call.
    AlreadyResolved = 15,
    /// The bonus pool holds less than the bonus owed.
    InsufficientBonusPool = 16,
    /// No bonus token has been configured.
    BonusTokenNotSet = 17,
    InvalidAmount = 18,
}
