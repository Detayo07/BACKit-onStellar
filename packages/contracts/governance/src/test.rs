#![cfg(test)]

extern crate std;

use crate::{Governance, GovernanceClient};
use soroban_sdk::{
    testutils::Address as _,
    Address, Env, Symbol,
};

#[test]
fn test_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let contract_id = env.register(Governance, ());
    let client = GovernanceClient::new(&env, &contract_id);
    client.initialize(&admin, &10, &5000, &1000, &1000, &5000);

    let result = client.try_initialize(&admin, &10, &5000, &1000, &1000, &5000);
    assert!(result.is_err());
}

#[test]
fn test_propose_and_vote() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    let target = Address::generate(&env);

    let contract_id = env.register(Governance, ());
    let client = GovernanceClient::new(&env, &contract_id);
    client.initialize(&admin, &1, &1000, &1000, &1000, &5000);

    client.set_reputation(&admin, &proposer, &5, &1_000_000_000);
    client.set_reputation(&admin, &voter, &3, &500_000_000);

    let pid = client.propose_change(&proposer, &target, &Symbol::new(&env, "set_param"));
    assert_eq!(pid, 1);

    client.vote(&voter, &pid, &true);

    let proposal = client.get_proposal_view(&1);
    assert!(proposal.for_votes > 0);
}

// ── SC-032: treasury fee split ───────────────────────────────────────────────

use crate::errors::GovernanceError;
use crate::treasury::Bucket;
use soroban_sdk::{
    token::{Client as TokenClient, StellarAssetClient},
    vec,
};

struct Treasury<'a> {
    env: Env,
    client: GovernanceClient<'a>,
    token: Address,
    payer: Address,
    signers: [Address; 3],
}

fn treasury<'a>() -> Treasury<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let signers = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];

    let id = env.register(Governance, ());
    let client = GovernanceClient::new(&env, &id);
    client.initialize(&admin, &1, &1000, &1000, &1000, &5000);
    client.init_treasury(&admin, &vec![&env, signers[0].clone(), signers[1].clone(), signers[2].clone()], &2);

    let token = env.register_stellar_asset_contract_v2(admin).address();
    let payer = Address::generate(&env);
    StellarAssetClient::new(&env, &token).mint(&payer, &1_000_000);
    Treasury { env, client, token, payer, signers }
}

fn two_signers(t: &Treasury) -> soroban_sdk::Vec<Address> {
    vec![&t.env, t.signers[0].clone(), t.signers[1].clone()]
}

#[test]
fn default_split_is_50_25_15_10() {
    let t = treasury();
    assert_eq!(t.client.fee_split(), vec![&t.env, 5_000u32, 2_500, 1_500, 1_000]);
}

#[test]
fn ingesting_a_fee_fills_all_four_buckets_and_burns_ten_percent() {
    let t = treasury();
    let asset = TokenClient::new(&t.env, &t.token);

    let parts = t.client.ingest_fee(&t.payer, &t.token, &10_000);

    assert_eq!(parts, vec![&t.env, 5_000i128, 2_500, 1_500, 1_000]);
    assert_eq!(t.client.bucket_balance(&t.token, &Bucket::Treasury), 5_000);
    assert_eq!(t.client.bucket_balance(&t.token, &Bucket::CreatorIncentives), 2_500);
    assert_eq!(t.client.bucket_balance(&t.token, &Bucket::Buyback), 1_500);
    assert_eq!(t.client.total_burned(&t.token), 1_000);
    // The burn bucket really left circulation; the rest is held by the contract.
    assert_eq!(asset.balance(&t.client.address), 9_000);
    assert_eq!(asset.balance(&t.payer), 1_000_000 - 10_000);
}

#[test]
fn rounding_never_loses_or_creates_tokens() {
    let t = treasury();
    for amount in [1i128, 7, 99, 101, 12_345] {
        let parts = t.client.ingest_fee(&t.payer, &t.token, &amount);
        let sum: i128 = parts.iter().sum();
        assert_eq!(sum, amount);
    }
    // 7 -> creators 1, buyback 1, burn 0, treasury takes the remainder 5.
    let t2 = treasury();
    assert_eq!(
        t2.client.ingest_fee(&t2.payer, &t2.token, &7),
        vec![&t2.env, 5i128, 1, 1, 0]
    );
}

#[test]
fn ingest_rejects_non_positive_amounts() {
    let t = treasury();
    assert_eq!(
        t.client.try_ingest_fee(&t.payer, &t.token, &0),
        Err(Ok(GovernanceError::InvalidAmount))
    );
}

#[test]
fn the_split_changes_only_with_threshold_signers_and_a_valid_table() {
    let t = treasury();
    let new_split = vec![&t.env, 4_000u32, 3_000, 2_000, 1_000];

    // One signer is below the threshold of two.
    assert_eq!(
        t.client.try_set_fee_split(&vec![&t.env, t.signers[0].clone()], &new_split),
        Err(Ok(GovernanceError::NotEnoughSigners))
    );
    // The same signer twice still counts once.
    assert_eq!(
        t.client.try_set_fee_split(
            &vec![&t.env, t.signers[0].clone(), t.signers[0].clone()],
            &new_split
        ),
        Err(Ok(GovernanceError::NotEnoughSigners))
    );
    // A non-signer is rejected outright.
    let outsider = Address::generate(&t.env);
    assert_eq!(
        t.client.try_set_fee_split(&vec![&t.env, t.signers[0].clone(), outsider], &new_split),
        Err(Ok(GovernanceError::Unauthorized))
    );
    // A table that does not sum to 10 000 is rejected even with enough signers.
    assert_eq!(
        t.client.try_set_fee_split(&two_signers(&t), &vec![&t.env, 5_000u32, 2_500, 1_500, 999]),
        Err(Ok(GovernanceError::InvalidSplit))
    );
    assert_eq!(
        t.client.try_set_fee_split(&two_signers(&t), &vec![&t.env, 10_000u32, 0, 0]),
        Err(Ok(GovernanceError::InvalidSplit))
    );

    t.client.set_fee_split(&two_signers(&t), &new_split);
    assert_eq!(t.client.fee_split(), new_split);

    let parts = t.client.ingest_fee(&t.payer, &t.token, &10_000);
    assert_eq!(parts, vec![&t.env, 4_000i128, 3_000, 2_000, 1_000]);
}

#[test]
fn bucket_withdrawals_need_threshold_signers_and_cannot_exceed_the_balance() {
    let t = treasury();
    let to = Address::generate(&t.env);
    t.client.ingest_fee(&t.payer, &t.token, &10_000);

    assert_eq!(
        t.client.try_withdraw_bucket(
            &vec![&t.env, t.signers[2].clone()],
            &t.token,
            &Bucket::Buyback,
            &to,
            &100
        ),
        Err(Ok(GovernanceError::NotEnoughSigners))
    );
    assert_eq!(
        t.client.try_withdraw_bucket(&two_signers(&t), &t.token, &Bucket::Buyback, &to, &1_501),
        Err(Ok(GovernanceError::InsufficientBucket))
    );
    // Burned tokens no longer exist and cannot be withdrawn.
    assert_eq!(
        t.client.try_withdraw_bucket(&two_signers(&t), &t.token, &Bucket::Burn, &to, &1),
        Err(Ok(GovernanceError::InvalidAmount))
    );

    t.client.withdraw_bucket(&two_signers(&t), &t.token, &Bucket::Buyback, &to, &1_500);
    assert_eq!(TokenClient::new(&t.env, &t.token).balance(&to), 1_500);
    assert_eq!(t.client.bucket_balance(&t.token, &Bucket::Buyback), 0);
}

#[test]
fn treasury_setup_is_one_time_and_validated() {
    let t = treasury();
    let admin = Address::generate(&t.env);
    // Only the configured admin may set it up (the real admin already did).
    assert!(t
        .client
        .try_init_treasury(&admin, &vec![&t.env, t.signers[0].clone()], &1)
        .is_err());

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(Governance, ());
    let client = GovernanceClient::new(&env, &id);
    client.initialize(&admin, &1, &1000, &1000, &1000, &5000);
    let one = Address::generate(&env);
    assert_eq!(
        client.try_init_treasury(&admin, &vec![&env, one.clone()], &2),
        Err(Ok(GovernanceError::InvalidTreasuryConfig))
    );
    assert_eq!(
        client.try_init_treasury(&admin, &vec![&env, one.clone()], &0),
        Err(Ok(GovernanceError::InvalidTreasuryConfig))
    );
    client.init_treasury(&admin, &vec![&env, one.clone()], &1);
    assert_eq!(
        client.try_init_treasury(&admin, &vec![&env, one], &1),
        Err(Ok(GovernanceError::AlreadyInitialized))
    );
}
