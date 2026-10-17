use soroban_sdk::{Address, Env, Symbol};

pub fn emit_proposal_created(env: &Env, id: u64, proposer: &Address, target: &Address, function_name: &Symbol) {
    env.events().publish(
        ("governance", "ProposalCreated"),
        (id, proposer.clone(), target.clone(), function_name.clone()),
    );
}

pub fn emit_vote_cast(env: &Env, proposal_id: u64, voter: &Address, support: bool, power: i128) {
    env.events().publish(
        ("governance", "VoteCast"),
        (proposal_id, voter.clone(), support, power),
    );
}

pub fn emit_proposal_executed(env: &Env, id: u64) {
    env.events().publish(("governance", "ProposalExecuted"), (id,));
}

pub fn emit_delegation(env: &Env, delegator: &Address, delegate: &Address) {
    env.events().publish(
        ("governance", "DelegationChanged"),
        (delegator.clone(), delegate.clone()),
    );
}

pub fn emit_fee_split(env: &Env, token: &Address, amount: i128, split: &[i128; 4]) {
    env.events().publish(
        ("governance", "FeeSplit"),
        (token.clone(), amount, split[0], split[1], split[2], split[3]),
    );
}

pub fn emit_split_updated(env: &Env, bps: &[u32; 4]) {
    env.events().publish(
        ("governance", "FeeSplitUpdated"),
        (bps[0], bps[1], bps[2], bps[3]),
    );
}
