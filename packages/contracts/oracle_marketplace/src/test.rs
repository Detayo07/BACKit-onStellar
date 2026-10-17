#![cfg(test)]

extern crate std;

use crate::{
    types::{MarketplaceConfig, OracleProvider},
    OracleMarketplace, OracleMarketplaceClient,
};
use soroban_sdk::{
    testutils::Address as AddressTest,
    Address, BytesN, Env,
};

#[test]
fn initialize_marketplace() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let contract_id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &contract_id);
    client.initialize(&admin, &3600u64, &100u32);

    let config = client.get_config_view();
    assert_eq!(config.admin, admin);
    assert_eq!(config.cooldown_secs, 3600);
    assert_eq!(config.default_fee_bps, 100);
}

#[test]
fn register_and_deregister_oracle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    let contract_id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &contract_id);
    client.initialize(&admin, &3600u64, &100u32);

    client.register_oracle(&provider, &pubkey, &100u32, &0i128);

    let oracles = client.get_available_oracles();
    assert_eq!(oracles.len(), 1);

    client.deregister_oracle(&provider, &pubkey);

    let oracles = client.get_available_oracles();
    assert_eq!(oracles.len(), 0);
}

#[test]
fn select_oracle_for_call_and_rate() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    let contract_id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &contract_id);
    client.initialize(&admin, &3600u64, &100u32);

    client.register_oracle(&provider, &pubkey, &100u32, &0i128);
    client.select_oracle_for_call(&42u64, &pubkey);

    let selected = client.get_call_oracle(&42u64);
    assert_eq!(selected, Some(pubkey.clone()));

    client.rate_oracle(&user, &pubkey, &true);

    let metrics = client.get_oracle_metrics(&pubkey);
    assert_eq!(metrics, (0u64, 0u64));
}

// ── SC-029 speed multipliers / SC-030 liveness ───────────────────────────────

use crate::errors::OracleMarketplaceError;
use crate::performance::{self, HEARTBEAT_MAX_AGE_LEDGERS};
use soroban_sdk::testutils::Ledger as _;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

struct Market<'a> {
    env: Env,
    client: OracleMarketplaceClient<'a>,
    token: Address,
    provider: Address,
    pubkey: BytesN<32>,
}

fn market<'a>() -> Market<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[7u8; 32]);

    let id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &id);
    client.initialize(&admin, &3600u64, &100u32);
    client.register_oracle(&provider, &pubkey, &100u32, &0i128);

    let token = env.register_stellar_asset_contract_v2(admin.clone()).address();
    client.set_bonus_token(&token);
    StellarAssetClient::new(&env, &token).mint(&admin, &1_000_000);
    client.fund_bonus_pool(&admin, &100_000);

    Market { env, client, token, provider, pubkey }
}

fn select(m: &Market, call_id: u64) {
    m.client.select_oracle_for_call(&call_id, &m.pubkey);
}

#[test]
fn multiplier_tiers_match_the_latency_boundaries() {
    use performance::multiplier_bps as mult;
    assert_eq!(mult(1_000, 900), 15_000); // before expiry counts as instant
    assert_eq!(mult(1_000, 1_000), 15_000);
    assert_eq!(mult(1_000, 1_059), 15_000);
    assert_eq!(mult(1_000, 1_060), 12_000);
    assert_eq!(mult(1_000, 1_299), 12_000);
    assert_eq!(mult(1_000, 1_300), 10_000);
    assert_eq!(mult(1_000, 99_999), 10_000);
}

#[test]
fn fast_resolution_pays_a_fifty_percent_bonus_and_updates_the_score() {
    let m = market();
    select(&m, 1);
    let token = TokenClient::new(&m.env, &m.token);

    let bonus = m.client.record_resolution(&m.pubkey, &1, &1_000, &1_030, &1_000);

    assert_eq!(bonus, 500);
    assert_eq!(token.balance(&m.provider), 500);
    let perf = m.client.get_oracle_performance(&m.pubkey);
    assert_eq!(perf.resolutions, 1);
    assert_eq!(perf.bonus_resolutions, 1);
    assert_eq!(perf.bonus_paid, 500);
    assert_eq!(perf.score, 15_000);
    assert_eq!(m.client.get_oracle_metrics(&m.pubkey), (1, 0));
}

#[test]
fn medium_and_slow_resolutions_pay_less_or_nothing() {
    let m = market();
    select(&m, 1);
    select(&m, 2);
    let token = TokenClient::new(&m.env, &m.token);

    assert_eq!(m.client.record_resolution(&m.pubkey, &1, &1_000, &1_200, &1_000), 200);
    assert_eq!(m.client.record_resolution(&m.pubkey, &2, &1_000, &5_000, &1_000), 0);

    assert_eq!(token.balance(&m.provider), 200);
    let perf = m.client.get_oracle_performance(&m.pubkey);
    assert_eq!(perf.resolutions, 2);
    assert_eq!(perf.bonus_resolutions, 1);
    assert_eq!(perf.score, 12_000 + 10_000);
}

#[test]
fn a_resolution_can_only_be_recorded_once_by_the_selected_oracle() {
    let m = market();
    select(&m, 1);
    m.client.record_resolution(&m.pubkey, &1, &1_000, &1_030, &1_000);
    assert_eq!(
        m.client.try_record_resolution(&m.pubkey, &1, &1_000, &1_030, &1_000),
        Err(Ok(OracleMarketplaceError::AlreadyResolved))
    );

    // Call 2 was never routed to this oracle.
    assert_eq!(
        m.client.try_record_resolution(&m.pubkey, &2, &1_000, &1_030, &1_000),
        Err(Ok(OracleMarketplaceError::OracleNotSelectedForCall))
    );
}

#[test]
fn a_bonus_larger_than_the_pool_is_rejected() {
    let m = market();
    select(&m, 1);
    assert_eq!(
        m.client.try_record_resolution(&m.pubkey, &1, &1_000, &1_030, &1_000_000),
        Err(Ok(OracleMarketplaceError::InsufficientBonusPool))
    );
    assert_eq!(m.client.get_oracle_performance(&m.pubkey).resolutions, 0);
}

/// Extend the contract's instance TTL so ledger jumps in the tests do not archive it.
fn keep_alive(m: &Market) {
    m.env.as_contract(&m.client.address, || {
        m.env.storage().instance().extend_ttl(200_000, 200_000);
    });
}

fn advance(m: &Market, ledgers: u32) {
    m.env.ledger().with_mut(|l| l.sequence_number += ledgers);
}

#[test]
fn a_new_oracle_is_live_and_stays_live_for_24_hours() {
    let m = market();
    keep_alive(&m);
    assert!(m.client.is_oracle_live(&m.pubkey));

    advance(&m, HEARTBEAT_MAX_AGE_LEDGERS);
    assert!(m.client.is_oracle_live(&m.pubkey));
    assert_eq!(m.client.get_available_oracles().len(), 1);
}

#[test]
fn an_idle_oracle_is_dropped_from_routing_until_it_sends_a_heartbeat() {
    let m = market();
    keep_alive(&m);

    advance(&m, HEARTBEAT_MAX_AGE_LEDGERS + 1);
    assert!(!m.client.is_oracle_live(&m.pubkey));
    assert_eq!(m.client.get_available_oracles().len(), 0);
    assert_eq!(
        m.client.try_select_oracle_for_call(&9, &m.pubkey),
        Err(Ok(OracleMarketplaceError::OracleNotLive))
    );

    m.client.submit_heartbeat(&m.provider, &m.pubkey);

    assert!(m.client.is_oracle_live(&m.pubkey));
    assert_eq!(m.client.get_available_oracles().len(), 1);
    m.client.select_oracle_for_call(&9, &m.pubkey);
    assert_eq!(m.client.get_last_heartbeat(&m.pubkey), Some(m.env.ledger().sequence()));
}

#[test]
fn only_the_oracles_own_provider_can_send_its_heartbeat() {
    let m = market();
    let other = Address::generate(&m.env);
    assert_eq!(
        m.client.try_submit_heartbeat(&other, &m.pubkey),
        Err(Ok(OracleMarketplaceError::Unauthorized))
    );
    let unknown = BytesN::from_array(&m.env, &[9u8; 32]);
    assert_eq!(
        m.client.try_submit_heartbeat(&m.provider, &unknown),
        Err(Ok(OracleMarketplaceError::OracleNotFound))
    );
}
