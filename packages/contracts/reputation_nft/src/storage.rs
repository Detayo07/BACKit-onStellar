use crate::types::{Badge, ReputationConfig};
use soroban_sdk::{contracttype, Address, Env, Map, Symbol, Vec};

#[contracttype]
pub enum DataKey {
    Config,
    BadgeCounter,
    Badge(u64),
    UserBadges(Address),
    BadgeAwarded(Symbol, Address),
}

pub fn set_config(env: &Env, config: &ReputationConfig) {
    env.storage().instance().set(&DataKey::Config, config);
}

pub fn get_config(env: &Env) -> Option<ReputationConfig> {
    env.storage().instance().get(&DataKey::Config)
}

pub fn next_badge_id(env: &Env) -> u64 {
    let counter: u64 = env
        .storage()
        .instance()
        .get(&DataKey::BadgeCounter)
        .unwrap_or(0);
    let next_id = counter + 1;
    env.storage()
        .instance()
        .set(&DataKey::BadgeCounter, &next_id);
    next_id
}

pub fn set_badge(env: &Env, id: u64, badge: &Badge) {
    env.storage().instance().set(&DataKey::Badge(id), badge);
}

pub fn get_badge(env: &Env, id: u64) -> Option<Badge> {
    env.storage().instance().get(&DataKey::Badge(id))
}

pub fn add_user_badge(env: &Env, user: &Address, badge_id: u64) {
    let key = DataKey::UserBadges(user.clone());
    let mut list: Vec<u64> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    list.push_back(badge_id);
    env.storage().instance().set(&key, &list);
}

pub fn get_user_badge_ids(env: &Env, user: &Address) -> Vec<u64> {
    let key = DataKey::UserBadges(user.clone());
    env.storage()
        .instance()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn remove_badge(env: &Env, id: u64) {
    env.storage().instance().remove(&DataKey::Badge(id));
}

pub fn remove_user_badge(env: &Env, user: &Address, badge_id: u64) {
    let key = DataKey::UserBadges(user.clone());
    let list: Vec<u64> = env
        .storage()
        .instance()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    let mut kept = Vec::new(env);
    for id in list.iter() {
        if id != badge_id {
            kept.push_back(id);
        }
    }
    env.storage().instance().set(&key, &kept);
}

pub fn unmark_badge_awarded(env: &Env, badge_type: &Symbol, user: &Address) {
    env.storage()
        .instance()
        .remove(&DataKey::BadgeAwarded(badge_type.clone(), user.clone()));
}

pub fn has_badge_type(env: &Env, badge_type: &Symbol, user: &Address) -> bool {
    let key = DataKey::BadgeAwarded(badge_type.clone(), user.clone());
    env.storage().instance().get(&key).unwrap_or(false)
}

pub fn mark_badge_awarded(env: &Env, badge_type: &Symbol, user: &Address) {
    let key = DataKey::BadgeAwarded(badge_type.clone(), user.clone());
    env.storage().instance().set(&key, &true);
}
