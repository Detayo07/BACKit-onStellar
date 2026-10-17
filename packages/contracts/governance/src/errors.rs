use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum GovernanceError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ProposalNotFound = 4,
    AlreadyVoted = 5,
    ProposalInactive = 6,
    VotingPeriodEnded = 7,
    InsufficientReputation = 8,
    AlreadyExecuted = 9,
    ProposalNotPassed = 10,
    QuorumNotMet = 11,
    AlreadyDelegated = 12,
    SelfDelegation = 13,
    /// Fee split basis points do not sum to 10 000.
    InvalidSplit = 30,
    /// Fewer distinct treasury signers authorised the call than the threshold.
    NotEnoughSigners = 31,
    InvalidAmount = 32,
    /// The bucket holds less than the requested amount.
    InsufficientBucket = 33,
    InvalidTreasuryConfig = 34,
}
