#![cfg(test)]

extern crate std;

use crate::{ReputationNft, ReputationNftClient};
use soroban_sdk::{
    testutils::Address as AddressTest,
    symbol_short, Address, Env,
};

#[test]
fn initialize_reputation() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(ReputationNft, ());
    let client = ReputationNftClient::new(&env, &contract_id);
    client.initialize(&admin);

    let config = client.get_config_view();
    assert_eq!(config.admin, admin);
}

#[test]
fn award_badge_and_prevent_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let contract_id = env.register(ReputationNft, ());
    let client = ReputationNftClient::new(&env, &contract_id);
    client.initialize(&admin);

    let token_id = client.award_badge(
        &user,
        &symbol_short!("EARLY"),
        &soroban_sdk::String::from_str(&env, "ipfs://early"),
    );

    assert_eq!(token_id, 1);

    let badges = client.get_user_badges(&user);
    assert_eq!(badges.len(), 1);

    let result = client.try_transfer(&user, &Address::generate(&env), &token_id);
    assert!(result.is_err());
}

// ── SC-031: soulbound badges ─────────────────────────────────────────────────

use crate::errors::ReputationError;
use soroban_sdk::{Symbol, String as SorobanString};

fn nft<'a>() -> (Env, ReputationNftClient<'a>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(ReputationNft, ());
    let client = ReputationNftClient::new(&env, &id);
    client.initialize(&admin);
    (env, client, admin)
}

fn uri(env: &Env, s: &str) -> SorobanString {
    SorobanString::from_str(env, s)
}

#[test]
fn mint_issues_unique_badges_with_tier_and_metadata() {
    let (env, client, _admin) = nft();
    let user = Address::generate(&env);
    let parlay = Symbol::new(&env, "PARLAY_CHAMPION");
    let accuracy = Symbol::new(&env, "TOP_TIER_ACCURACY");

    let a = client.mint(&user, &parlay, &3, &uri(&env, "ipfs://parlay"));
    let b = client.mint(&user, &accuracy, &5, &uri(&env, "ipfs://accuracy"));

    assert_eq!((a, b), (1, 2));
    assert_eq!(client.balance(&user), 2);
    assert_eq!(client.owner_of(&a), user);
    assert_eq!(client.metadata(&a), (uri(&env, "ipfs://parlay"), 3));
    assert_eq!(client.get_badge(&b).badge_type, accuracy);
}

#[test]
fn a_badge_cannot_be_minted_twice_to_the_same_holder() {
    let (env, client, _admin) = nft();
    let user = Address::generate(&env);
    let parlay = Symbol::new(&env, "PARLAY_CHAMPION");
    client.mint(&user, &parlay, &1, &uri(&env, "ipfs://a"));
    assert_eq!(
        client.try_mint(&user, &parlay, &2, &uri(&env, "ipfs://b")),
        Err(Ok(ReputationError::BadgeAlreadyAwarded))
    );
    assert_eq!(client.balance(&user), 1);
}

#[test]
fn mint_validates_badge_id_and_tier() {
    let (env, client, _admin) = nft();
    let user = Address::generate(&env);
    assert_eq!(
        client.try_mint(&user, &Symbol::new(&env, "NOT_A_BADGE"), &1, &uri(&env, "x")),
        Err(Ok(ReputationError::InvalidBadgeType))
    );
    let parlay = Symbol::new(&env, "PARLAY_CHAMPION");
    assert_eq!(
        client.try_mint(&user, &parlay, &0, &uri(&env, "x")),
        Err(Ok(ReputationError::InvalidTier))
    );
    assert_eq!(
        client.try_mint(&user, &parlay, &6, &uri(&env, "x")),
        Err(Ok(ReputationError::InvalidTier))
    );
}

#[test]
fn every_transfer_path_is_blocked() {
    let (env, client, _admin) = nft();
    let user = Address::generate(&env);
    let other = Address::generate(&env);
    let id = client.mint(&user, &Symbol::new(&env, "PARLAY_CHAMPION"), &1, &uri(&env, "x"));

    assert_eq!(
        client.try_transfer(&user, &other, &id),
        Err(Ok(ReputationError::NonTransferable))
    );
    assert_eq!(
        client.try_transfer_from(&other, &user, &other, &id),
        Err(Ok(ReputationError::NonTransferable))
    );
    // Ownership is unchanged.
    assert_eq!(client.owner_of(&id), user);
    assert_eq!(client.balance(&other), 0);
}

#[test]
fn admin_can_revoke_and_the_badge_disappears() {
    let (env, client, _admin) = nft();
    let user = Address::generate(&env);
    let parlay = Symbol::new(&env, "PARLAY_CHAMPION");
    let id = client.mint(&user, &parlay, &2, &uri(&env, "x"));

    client.revoke(&id);

    assert_eq!(client.balance(&user), 0);
    assert_eq!(client.try_owner_of(&id), Err(Ok(ReputationError::BadgeNotFound)));
    assert_eq!(client.get_user_badges(&user).len(), 0);
    // Revoked means it can be earned again.
    client.mint(&user, &parlay, &1, &uri(&env, "y"));
    assert_eq!(client.balance(&user), 1);
}

#[test]
fn revoking_an_unknown_badge_fails() {
    let (_env, client, _admin) = nft();
    assert_eq!(client.try_revoke(&99), Err(Ok(ReputationError::BadgeNotFound)));
}
