//! Public interface for other contracts that integrate with the call registry
//! (DAO treasuries, parlays, index funds) -- SC-019.
//!
//! `#[contractclient]` generates [`CallRegistryExternalClient`], which a
//! dependent contract can construct from the registry's address to create
//! calls and stake without depending on this crate's implementation.
use soroban_sdk::{contractclient, Address, Env};

use crate::errors::CallRegistryError;
use crate::types::Call;

#[contractclient(name = "CallRegistryExternalClient")]
pub trait CallRegistryTrait {
    /// Stake `amount` on `position` of a call, paid by the caller.
    fn stake_on_call(
        env: Env,
        staker: Address,
        call_id: u64,
        amount: i128,
        position: u32,
    ) -> Result<Call, CallRegistryError>;

    /// Stake on behalf of `beneficiary`; `funder` authorises and pays.
    fn stake_on_behalf(
        env: Env,
        funder: Address,
        beneficiary: Address,
        call_id: u64,
        amount: i128,
        position: u32,
    ) -> Result<Call, CallRegistryError>;

    /// Read a call.
    fn get_call(env: Env, call_id: u64) -> Result<Call, CallRegistryError>;
}
