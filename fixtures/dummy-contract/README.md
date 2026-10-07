# Dummy Contract Fixture

This is a minimal `no_std` Soroban contract used as a fixture for testing the `soroban-cost-profiler`.
It contains a simple `compute_heavy_loop` function that allows us to test the CPU instruction counting and execution tracing without depending on complex external logic.

This library is standalone and does not depend on the profiler crate, avoiding circular dependencies.

## Debug info

`fixtures/build.sh` builds this crate with the root manifest's
`[profile.release.package.dummy-contract] debug = "line-tables-only"`, so the output carries
`.debug_info`, `.debug_line`, `.debug_abbrev`, `.debug_str` and `.debug_ranges` — about 619 KB of
them against a 488-byte code section, because a `std`-linked release build emits debug info for
every inlined dependency, not just for these functions.

That is deliberate and it has two consequences. This artifact is a *profiling input*: source
mapping needs line tables to turn a program counter into a `file:line`, and nothing here deploys
it. And it is too big for git, so it stays a CI build product (`build-fixture`), while the small
DWARF test data a unit test can `include_bytes!` lives next door in `fixtures/dwarf_probe/`.
