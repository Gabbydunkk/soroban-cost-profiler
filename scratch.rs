use soroban_env_host::{Host, storage::Storage};
use soroban_ledger_snapshot::LedgerSnapshot;
use std::rc::Rc;

fn main() {
    let snapshot = LedgerSnapshot::default();
    let host = Host::with_storage_and_budget(
        Storage::with_enforcing_footprint_and_map(Default::default(), Default::default(), Default::default()).unwrap(),
        soroban_env_host::budget::Budget::default()
    );
}
