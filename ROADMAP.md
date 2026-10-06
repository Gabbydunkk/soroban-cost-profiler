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
- [x] **Call/Return Tracking:** Record entry and exit events for the WASM call the profiler
  initiates. Not the whole call tree: `wasmi` 2.0 reports no inner WASM-to-WASM calls, so the
  entry/exit pair is the outer one only (pinned by
  `only_the_outer_invocation_is_recorded_as_a_boundary`, and documented on `invoke_function`).
- [x] **Document Hook Reality:** `invoke_function`'s rustdoc claimed every WASM/host entry and
  exit becomes an event, which contradicted both the engine and our own probes. Rewritten to
  state which boundaries fire, that host boundaries nest correctly while WASM ones do not, and
  why every event is recorded at `pc = 0`.

## Phase 3: DWARF Source Mapping
- [ ] **Add Dependencies:** Add `addr2line` and `gimli` for debug info parsing. Neither is a new
  download: both are already in `Cargo.lock` at `addr2line 0.25.1` / `gimli 0.32.3` (pulled in by
  `backtrace`), and `addr2line` with `default-features = false, features = ["std"]` builds without
  `cpp_demangle` or `object`.
- [ ] **Enable Debug Info for the Fixture:** A prerequisite found while documenting
  `src/source_map.rs` (#50): `fixtures/build.sh` inherits the root `[profile.release]`, which sets
  only `opt-level = "z"`, so `dummy_contract.wasm` ships with **no `.debug_*` section at all**. A
  `debug = "line-tables-only"` (or `2`) profile setting for the fixture is required before any
  test can assert a `file:line`. Whether it survives `stellar contract build`/`wasm-opt` is the
  open Question in Phase 3 Issue 0.
- [ ] **Load DWARF Info:** Parse the `.debug_info` and `.debug_line` sections of the loaded WASM binary in `src/source_map.rs`. Verified against a real build: Rust emits DWARF as *individual* WASM custom sections (`.debug_abbrev`, `.debug_info`, `.debug_str`, `.debug_line`, `.debug_loc`, `.debug_ranges`), and `addr2line::Context::from_sections` accepts their raw bytes, so this is a section scan with no hand-written DWARF parsing — which is what keeps `AGENTS.md`'s "no custom `gimli` parsers" rule satisfiable.
- [ ] **Address Resolution:** Implement the `resolve(pc)` function to translate a WASM Program Counter to a Rust `file:line` frame. Blocked upstream, not by DWARF: `wasmi` 2.0 gives a call hook no program counter and has no instruction hook, so every event the tracer records is at `pc = 0` — see the tracer's `invoke_function` docs and `only_the_outer_invocation_is_recorded_as_a_boundary`. Verified separately that DWARF line tables are code-section-relative (offsets `4`/`8`/`16`/`64` resolved to `src/lib.rs:2` and `:5`), so the missing piece is a real offset from the engine, plus Issue 22's offset translation once there is one.

## Phase 4: Aggregation & Formatting
- [x] **Tree Building:** Implement `ProfileAggregator` to consume the raw `TraceEvent` stream and build a `CallStackNode` tree (#52). Open frames live on a stack, so accounting allocates per call rather than per instruction (`AGENTS.md`'s OOM constraint); a frame that meets another of the same name pools into it, the way flamegraph consumers collapse repeated stack frames. Trapped runs keep their still-open frames and an unmatched `Return` is ignored, so a partial trace still renders.
- [x] **Cost Math:** Exclusive cost accumulates on the innermost open frame; a `Call`'s delta is charged to its caller (or, for the stream's first boundary, to the frame it opens); host cost lands in its own frame because the tracer records zero on entry and the whole budget delta on return. `inclusive_*` is filled by one post-order pass over the finished tree, so `inclusive == exclusive + sum(children.inclusive)` holds at every node by construction — asserted for every frame in a mixed WASM/host/recursion trace.
- [x] **Formatting:** Implement `OutputFormatter` to serialize the tree into the standard `.folded` collapsed stack format — written with #44/#53 and tested there; what was missing was an input, which Stage 3 now provides, so the call is live in `profile()` rather than commented out.
- [x] **Pipeline Wiring:** `src/main.rs`'s `profile()` now runs all four stages in order instead of commenting the last two out, and `tests/integration.rs` hands a traced run through tracer → aggregator → formatter, reading the result back with `parse_folded`. The tree is a placeholder until Phases 2 and 3 give it real boundaries and names, so the end-to-end output is one unresolved `wasm[0]` frame holding the total.
- [x] **Differential Comparison:** Diff two `.folded` artifacts into `<stack> <baseline> <current>` lines for `flamegraph.pl --diff`, with the red/blue/neutral classification tested in `src/formatter.rs` and `tests/differential.rs` (#44). Rendering stays an external step: `AGENTS.md` cuts SVG/inferno from the MVP.

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
- [x] Add comprehensive unit tests for `src/models.rs` (#49) — the derived semantics Phase 4 depends on: distinct `EventType` variants, field-by-field `TraceEvent` equality, `SourceFrame`'s independent `Option` location fields, and `CallStackNode` children keyed by function name.
- [x] Add comprehensive unit tests for `tests/integration.rs` (#45) — the seams around a run: `parse_module`/`load_wasm_file` error paths, fuel metering actually enabled, and the sampling and ceiling knobs.
- [x] Refactor and modularize complex logic in `src/main.rs` (#48) — each pipeline stage now constructed by its own function behind a `profile()` harness, which is also the first test to execute the binary's code at all.
- [x] Improve inline documentation and comments in `src/source_map.rs` (#50) — the stage's contract, what Phase 3 will hold, and the facts checked against real builds (section names, `from_sections` needs no `object`, code-section-relative addresses, the fixture currently shipping no DWARF). Documentation plus two tests pinning that construction tolerates a debug-info-free binary and that an unattributable `pc` yields no frame; `resolve()` itself stays blocked on the engine giving the tracer a real offset.

- [x] Implement and refactor `src/aggregator.rs` (#52) — `aggregate()` was `unimplemented!()`; it now folds the flat event stream into the tree Phase 4 needs, which is also what the two open Phase 4 boxes above describe. 13 unit tests cover cost attribution, nesting, host frames, pooling, trapped runs, and the inclusive invariant.

### Blocked on unimplemented code
The quality-issue bank (#45-#60) was generated per file, but several targets are still
scaffolds, so their ask has nothing to act on yet. Revisit after the phase that
implements the file:

- `src/source_map.rs` (#60 refactor) — a documented 17-line stub until Phase 3 implements
  `resolve()`; #50 documented it rather than restructuring it, because there is no logic yet to
  restructure.
- `src/models.rs` (#59 perf) — derive-only data structures; no loops or clones to remove,
  and the issue forbids changing the public API.
- `src/lib.rs` (#51 perf) — module declarations only.

### Not applicable as written
Two bank items ask for something that would make the code worse, so they are recorded here
rather than "solved" with a cosmetic diff:

- `fixtures/dummy-contract/src/lib.rs` (#47 perf) — the deliberately expensive loops *are*
  the fixture: `compute_heavy_loop` and `memory_heavy_loop` exist to produce measurable cost
  for the profiler. Optimizing them would remove the signal every trace test depends on, and
  #57 pinned their exact arithmetic.
- `tests/integration.rs` (#55 perf) — a test file with no loop, clone, or allocation to
  remove. #45 expanded it; there is nothing for a performance pass to do to it.

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
