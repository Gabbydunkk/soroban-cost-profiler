//! Binary entry point for `soroban-cost-profiler`.
//!
//! The profiler is a four-stage pipeline, and `main` is the wiring diagram for it:
//!
//! 1. **Trace** — run the contract's WASM under the instrumented engine and collect
//!    a flat stream of [`TraceEvent`]s (`tracer`).
//! 2. **Symbolize** — turn each event's program counter into a `file:line` frame
//!    (`source_map`).
//! 3. **Aggregate** — fold the flat stream into a [`CallStackNode`] tree carrying
//!    inclusive/exclusive costs (`aggregator`).
//! 4. **Format** — serialize the tree as `.folded` collapsed stacks for external
//!    viewers such as speedscope (`formatter`).
//!
//! Stages 2 and 3 are still scaffolds (Phase 3 and Phase 4 of `ROADMAP.md`), so the
//! calls that depend on them are commented out rather than deleted: they document the
//! intended call order, and uncommenting each line is the completion criterion for its
//! phase. The `_`-prefixed bindings exist for the same reason — they keep the type
//! plumbing compiling as an early warning when a stage's signature changes.
//!
//! Each stage's construction lives in its own function so that wiring Phase 3 or Phase 4
//! in is a one-line change at the call site, and so the placeholder input each stage needs
//! today has a documented home instead of sitting inline in `main`.
//!
//! [`TraceEvent`]: soroban_cost_profiler::models::TraceEvent
//! [`CallStackNode`]: soroban_cost_profiler::models::CallStackNode
use soroban_cost_profiler::aggregator::ProfileAggregator;
// Stage 4 waits on Stage 3: uncomment once `aggregate()` returns a tree.
// use soroban_cost_profiler::formatter::OutputFormatter;
use soroban_cost_profiler::source_map::SourceMapper;
use soroban_cost_profiler::tracer::ExecutionTracer;

/// Stage 1: build a tracer carrying the MVP sampling and instruction-ceiling defaults.
///
/// The engine hooks that feed it are installed by `tracer::invoke_function`, which needs a
/// `Store<ProfilerState>` rather than a bare tracer — that wiring lands with Phase 2's CLI.
fn initialize_tracer() -> ExecutionTracer {
    ExecutionTracer::new()
}

/// Stage 2: build the source mapper for the target WASM binary.
///
/// Empty bytes are a placeholder until the binary is read from disk, so `resolve()` has
/// nothing to map and returns `None`. Phase 3 replaces this with `load_wasm_file` output.
fn load_source_mapper() -> SourceMapper {
    SourceMapper::new(&[])
}

/// Stage 3: build an empty aggregator.
fn initialize_aggregator() -> ProfileAggregator {
    ProfileAggregator::new()
}

/// Run the pipeline stages that exist today.
///
/// Currently a dry harness: it assembles each stage with placeholder input and exercises no
/// WASM. It takes no CLI arguments yet — flag parsing (`--wasm`, `--output`) is Phase 5 —
/// so the calls that would move data between stages stay commented out as their phases'
/// completion criteria.
fn profile() {
    // 1. Initialize tracer and execute WASM
    let mut _tracer = initialize_tracer();
    // let _events = _tracer.flush_trace();

    // 2. Load DWARF source map
    let _mapper = load_source_mapper();

    // 3. Aggregate events into call tree
    let mut _aggregator = initialize_aggregator();
    // let _call_tree = _aggregator.aggregate(_events, &_mapper);

    // 4. Format and output
    // let _collapsed_stack = OutputFormatter::to_collapsed_stack(&_call_tree);
}

/// Print the MVP notice and run the harness.
fn main() {
    println!("soroban-cost-profiler MVP (Not yet implemented)");
    profile();
}

#[cfg(test)]
mod tests {
    use super::profile;

    /// The harness has no observable output yet, so the bar is that assembling the stages in
    /// the documented order runs to completion. This is the first test to execute `main`'s
    /// code at all: `cargo test` never calls `main`, so before this the binary target had no
    /// coverage whatsoever.
    #[test]
    fn assembling_the_stages_runs_to_completion() {
        profile();
    }
}
