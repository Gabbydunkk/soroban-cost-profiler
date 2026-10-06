# 🗺️ Soroban Cost Profiler: Development Roadmap

> **⚠️ CRITICAL RULE:** This document MUST be updated whenever a contribution is made to the repository. If you finish a task, check it off here and update the progress.

## Phase 1: Core Scaffolding & Setup ✅
- [x] Create repository, README, and AGENTS.md instructions.
- [x] Draft PRD and Architecture documents.
- [x] Scaffold initial Rust pipeline modules (`tracer`, `aggregator`, `source_map`, `formatter`).
- [x] Define core data models (`TraceEvent`, `CallStackNode`).
- [x] Setup Tollcraft Org Landing Page.
- [x] Implement interactive demo animations for Linter and Assert tabs.
- [x] Implement Light/Dark mode toggle button in the navbar.

## Phase 2: Execution Tracing (In Progress 🚧)
- [x] **SPIKE:** Investigate `soroban-env-host` Budget API limitations.
- [x] **WASM Engine Setup:** Import `soroban-env-host` and `wasmi` as dependencies.
- [x] **Fixture Compilation:** Add a `fixtures/dummy-contract` Soroban contract with a `compute_heavy_loop` function and workspace integration.
- [x] **Fixture Documentation:** Add README and doc comments to dummy-contract fixture.
- [x] **Fixture Script:** Add `fixtures/build.sh` compile script.
- [x] **Memory Fixture:** Add `memory_heavy_loop` to dummy contract.
- [x] **Tracer State:** Scaffold `ExecutionTracer` state and `TraceEvent` structures.
- [x] **Instruction Metering:** Enable fuel consumption in the `wasmi` engine setup.
- [x] **WASM Parser:** Implement a WASM file loader and `wasmi` module parser.
- [x] **Host Setup:** Scaffold the native `soroban_env_host::Host` proxy for the tracer.
- [x] **Trace Data Models:** Write detailed doc comments for `TraceEvent` structures.
- [x] **Architecture Docs:** Create the internal `tracer_architecture.md` document explaining the sampling mechanics.
- [x] **Mock Host:** Scaffold `MockHost` struct for the environment.
- [x] **Module Instantiation:** Implement `instantiate_module` and link imports.
- [x] **Function Invocation:** Implement `invoke_function` for named exports.
- [x] **Memory Cost:** Track `mem_cost` alongside `cpu_cost`.
- [x] **Host Boundaries:** Distinguish host call boundaries from WASM boundaries.
- [x] **Tracer Hooks:** Implement the `wasmi` execution hooks in `src/tracer.rs` to intercept instructions.
- [x] **Instruction Counting:** Accurately measure and record CPU cost and `pc` at every step.
- [x] **Call/Return Tracking:** Record entry and exit events for WASM function calls.

## Phase 3: DWARF Source Mapping
- [ ] **Add Dependencies:** Add `addr2line` and `gimli` for debug info parsing.
- [ ] **Load DWARF Info:** Parse the `.debug_info` and `.debug_line` sections of the loaded WASM binary in `src/source_map.rs`.
- [ ] **Address Resolution:** Implement the `resolve(pc)` function to translate a WASM Program Counter to a Rust `file:line` frame.

## Phase 4: Aggregation & Formatting
- [ ] **Tree Building:** Implement `ProfileAggregator` to consume the raw `TraceEvent` stream and build a `CallStackNode` tree.
- [ ] **Cost Math:** Calculate `inclusive_cpu` and `exclusive_cpu` correctly during aggregation.
- [ ] **Formatting:** Implement `OutputFormatter` to serialize the tree into the standard `.folded` collapsed stack format.

## Phase 5: CLI & Edge Cases (MVP Completion)
- [ ] **CLI Parsing:** Add `clap` to `src/main.rs` to accept `--wasm`, `--output`, and test arguments.
- [ ] **Panic Handling:** Ensure the aggregator flushes and formats the trace even if the contract panics mid-execution.
- [ ] **Infinite Loop Protection:** Enforce a hard ceiling (e.g. 100M instructions) to halt tracing and prevent OOM crashes.
- [ ] **Documentation:** Update README with usage examples and CLI flag details.
- [x] **FAQ:** Design and implement FAQ section for GitHub Pages (`docs/index.html`).

## Tooling & Agent Setup
- [x] Install `agentic-awesome-skills` to `.agents/` for enhanced AI workflows.
- [x] Install official Anthropic skills and plugins from `claude-plugins-official`.
- [x] Generate `INSTALLED_SKILLS.md` catalog detailing all loaded agents and plugins.
- [x] Install `frontend-design` (anthropics/skills) and `design-taste-frontend` (leonxlnx/taste-skill) UI/UX skills for site audits.

## Website Polish (Landing Page Audit) ✅
- [x] **Fix marquee full-width bug:** Missing `</div>` caused `.marq-wrap` to nest inside `.wrap.hero__grid` and render as a 649px grid column instead of a full-width band.
- [x] **Light-theme contrast:** Override `--cyan`/`--magenta`/`--faint`/`--t2` with darker variants; white text on primary CTA; visible ghost-button border; visible `flame--4` bar.
- [x] **Dark-theme contrast:** Deepen `--violet` for primary CTA (AA 4.5:1) and lighten `--faint` for small mono labels.
- [x] **Responsive nav:** Tighten `.nav__links` gap to fix overflow at ~768px; add `scroll-padding-top` for anchored sections.
- [x] **Polish:** Add inline SVG favicon, `color-scheme` for native scrollbars, remove dead CSS (`.term__flame-block`, `.card`), clean duplicate rule, swap visible em-dashes for commas/parens.
- [x] **Social sharing:** Add branded 1200x630 Open Graph image (`docs/og-image.png`) plus `og:image`/`twitter:card` (summary_large_image) meta tags with alt text.
- [x] **Favicon:** Replace the inline SVG favicon with the Tollcraft org profile picture (`docs/favicon.png`, downloaded from GitHub avatars and converted to PNG), plus an `apple-touch-icon` link.

## Phase 6: Code Quality & Refactoring ✅
- [x] Refactor and modularize complex logic in `src/tracer.rs` (#64)
- [x] Review and optimize performance/allocations in `src/formatter.rs` (#63)
- [x] Improve inline documentation and comments in `src/aggregator.rs` (#62)
- [x] Add comprehensive unit tests for `src/lib.rs` (#61)
- [x] Add comprehensive unit tests for `src/formatter.rs` (#53)
- [x] Improve inline documentation and comments in `src/tracer.rs` (#54)
- [x] Add comprehensive unit tests for `fixtures/dummy-contract/src/lib.rs` (#57)
- [x] Improve inline documentation and comments in `src/main.rs` (#58)

### Blocked on unimplemented code
The quality-issue bank (#45-#60) was generated per file, but several targets are still
scaffolds, so their ask has nothing to act on yet. Revisit after the phase that
implements the file:

- `src/source_map.rs` (#60 refactor, #50 docs) — 17-line stub; `resolve()` returns `None`
  until Phase 3 (DWARF parsing, #43).
- `src/aggregator.rs` (#52 refactor) — `aggregate()` is `unimplemented!()` until Phase 4.
- `src/models.rs` (#59 perf) — derive-only data structures; no loops or clones to remove,
  and the issue forbids changing the public API.
- `src/lib.rs` (#51 perf) — module declarations only.

## Metering Probes (`tests/meter_probe.rs`)
- [x] Create the probe suite — `tests/meter_probe.rs` did not exist, which is why #46 and #56 had nothing to act on.
- [x] Refactor and modularize complex logic in `tests/meter_probe.rs` (#56) — shared `Probe` harness, one job per test, WASM encoding isolated in `mod probe_module`.
- [x] Improve inline documentation and comments in `tests/meter_probe.rs` (#46) — byte-level WASM annotations and the reason behind every assertion.

### Findings the probes surfaced, both blocking Phase 4
- **Internal WASM calls are not traced.** `wasmi` 2.0's `Store::call_hook` fires only for the
  host-initiated call: `probe()` calling `work()` twice yields one Call/Return pair, not three.
  Phase 4's call tree cannot be rebuilt from boundaries the engine never reports. Pinned by
  `only_the_outer_invocation_is_recorded_as_a_boundary`.
- **The instruction ceiling cannot halt a run.** `invoke_function` discards the `Err` returned by
  `record_step` once the ceiling is passed, so a runaway contract runs to completion. Pinned by
  `the_instruction_ceiling_does_not_stop_execution`.
- **A trapped run keeps its trace.** An out-of-fuel contract still yields the boundaries crossed
  before the trap, which is what Phase 5's panic handling needs.
