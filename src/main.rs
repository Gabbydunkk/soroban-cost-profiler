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
//! [`TraceEvent`]: soroban_cost_profiler::models::TraceEvent
//! [`CallStackNode`]: soroban_cost_profiler::models::CallStackNode
use soroban_cost_profiler::aggregator::ProfileAggregator;
// Stage 4 waits on Stage 3: uncomment once `aggregate()` returns a tree.
// use soroban_cost_profiler::formatter::OutputFormatter;
use soroban_cost_profiler::source_map::SourceMapper;
use soroban_cost_profiler::tracer::ExecutionTracer;

/// Assemble the pipeline stages and run them over a target WASM binary.
///
/// Currently a dry harness: it constructs each stage with placeholder input and
/// prints a notice. It takes no CLI arguments yet — flag parsing (`--wasm`,
/// `--output`) is Phase 5.
fn main() {
    println!("soroban-cost-profiler MVP (Not yet implemented)");

    // 1. Initialize tracer and execute WASM
    let mut _tracer = ExecutionTracer::new();
    // let events = tracer.trace();

    // 2. Load DWARF source map. Empty bytes are a placeholder until the binary is
    // read from disk, so `resolve()` has nothing to map and returns `None`.
    let _mapper = SourceMapper::new(&[]);

    // 3. Aggregate events into call tree
    let mut _aggregator = ProfileAggregator::new();
    // let call_tree = aggregator.aggregate(events, &mapper);

    // 4. Format and output
    // let collapsed_stack = OutputFormatter::to_collapsed_stack(&call_tree);
    // println!("{}", collapsed_stack);
}
