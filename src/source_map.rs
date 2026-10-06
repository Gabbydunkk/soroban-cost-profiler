use crate::models::SourceFrame;

/// Stage 2 of the pipeline: turn a program counter into a Rust source frame.
///
/// # Why the pipeline needs it
///
/// Tracer events carry a `pc`, and a bare `pc` is not actionable. A flamegraph frame named
/// `wasm[17]` tells a contract author nothing; the same frame named
/// `compute_heavy_loop (src/lib.rs:5)` names the loop to fix. This type is the only place
/// that translation happens, which is why [`ProfileAggregator::aggregate`] borrows a
/// `&SourceMapper` instead of naming frames itself, and why [`SourceFrame`] splits the name
/// from the location: a stripped binary can still yield a function name, and a line table
/// can still yield a line with no name.
///
/// # Status: scaffold (Phase 3 of `ROADMAP.md`)
///
/// `new` stores nothing and `resolve` resolves nothing, so Stages 3 and 4 can be written
/// against a stable surface while the DWARF work lands behind it. Two consequences callers
/// have to live with today:
///
/// * Every `pc` resolves to `None`, so an aggregated tree can only be keyed by the names the
///   tracer already knows.
/// * Construction is infallible, so "this binary has no debug info" and "this address is not
///   inside any range" are not yet distinguishable. [`docs/issues/phase_3_issues.md`]
///   (issues 3 and 8) plans a fallible `load_dwarf_sections` whose error tells the user to
///   check their compilation flags; making `new` return `Result` is that issue's call to
///   make, not a change to smuggle in here.
///
/// # What it will hold, and the facts that shape it
///
/// An `addr2line::Context` over the DWARF of the binary that was loaded. The following were
/// checked against real builds rather than assumed, because each one decides how much code
/// Phase 3 is:
///
/// * **Rust emits DWARF as separate custom sections, so Phase 3 is a section walk.** A
///   `wasm32-unknown-unknown` release build of a crate with `debug = 2` in
///   `[profile.release]` carries `.debug_abbrev`, `.debug_info`, `.debug_str`,
///   `.debug_line`, `.debug_loc` and `.debug_ranges` as individual WASM custom sections
///   (alongside `name`, `producers` and `target_features`). They are not merged into a
///   single `DWARF` section, so `ROADMAP.md`'s "parse the `.debug_info` and `.debug_line`
///   sections" is literally right.
/// * **`AGENTS.md`'s "no custom DWARF parsing" rule is satisfiable.** `addr2line` 0.25's
///   [`addr2line::Context::from_sections`] takes each section as a `gimli` reader over raw
///   bytes and needs no `object`/`memmap2` file wrapper, so the only hand-written code is
///   the section scan itself. Both `addr2line` 0.25.1 and `gimli` 0.32.3 are already in
///   `Cargo.lock` (pulled in by `backtrace`) with default features off, so using that
///   constructor adds no new transitive dependencies — and keeps `cpp_demangle` out of the
///   graph, which is right for a profiler that only demangles Rust symbols.
/// * **DWARF addresses are code-section-relative, and `wasmi` never hands us one.** In that
///   same build, looking up `4`, `8` and `16` returned `src/lib.rs:2` (the function's
///   signature) and `64` returned `src/lib.rs:5` (its loop body) — small integers that are
///   offsets into the code section, matching the issue bank's Issue 22. The reason this
///   stage still returns `None` is upstream: `wasmi` 2.0 gives a call hook no program
///   counter and offers no instruction hook at all, so every event `invoke_function`
///   records is written at `pc = 0`. There is nothing to look up until the engine gives the
///   tracer a real offset; see [`invoke_function`] for what the hooks can and cannot see.
/// * **The fixture has no DWARF to map yet.** `fixtures/build.sh` inherits the root
///   `[profile.release]`, which sets only `opt-level = "z"`, and the resulting
///   `dummy_contract.wasm` is 3.1 KB with no `.debug_*` section at all. A Phase 3 test that
///   asserts `file:line` needs debug info turned on for the fixture first — which is also
///   what Issue 0's "does it survive `wasm-opt`?" question is about.
/// * **The `name` section survives this build path, so Issue 26's fallback is real.** The
///   same DWARF-free fixture does carry a `name` custom section (1.6 KB, against a 488-byte
///   code section), which is what a function-name-only fallback would read when the line
///   tables are gone. That is the precedence the eventual docs should describe (DWARF ->
///   `name` -> function index, Issue 32) — noting that a plain `cargo build`, which is what
///   `build.sh` runs, does not strip it, while the `stellar contract build` path is
///   Issue 0's open question.
///
/// Two traps the first real `resolve` will hit, both seen in that probe:
///
/// * A location can carry a file and *no* line: address `32` came back as
///   `file: Some(".../src/lib.rs"), line: None`. `SourceFrame`'s `Option` location fields are
///   independent for exactly this reason, so fill them separately instead of treating a
///   missing line as line 0 or as a failed lookup.
/// * Paths are the absolute paths of whatever machine built the contract, and can contain
///   `../` segments. The probe resolved to `/private/tmp/<crate>/src/lib.rs`, and an address
///   in inlined dependency code resolved through
///   `/rustc/<hash>/library/compiler-builtins/.../../../../libm/src/math/...`. So anything
///   that groups or shortens frames by file must expect std/registry paths next to contract
///   paths, and a report is only readable next to the build that produced it.
///
/// [`ProfileAggregator::aggregate`]: crate::aggregator::ProfileAggregator::aggregate
/// [`invoke_function`]: crate::tracer::invoke_function
/// [`docs/issues/phase_3_issues.md`]: https://github.com/Tollcraft/soroban-cost-profiler/blob/main/docs/issues/phase_3_issues.md
/// [`addr2line::Context::from_sections`]: https://docs.rs/addr2line/0.25/addr2line/struct.Context.html#method.from_sections
pub struct SourceMapper {
    // TODO(Phase 3): the `addr2line::Context` built from the sections above, and — if Issue
    // 27 lands with it — the pc -> frame cache. `Context` is expensive to construct and
    // independent of the run, so it is owned here rather than rebuilt per lookup.
}

impl SourceMapper {
    /// Build a mapper for one already-loaded WASM binary.
    ///
    /// Takes bytes rather than a path because [`load_wasm_file`] has already read and
    /// validated the file, and the same bytes are handed to `parse_module` — reading twice
    /// would let the traced binary and the symbolized binary disagree.
    ///
    /// Infallible today, and `main` depends on that: `load_source_mapper` builds the
    /// stage-2 placeholder with `SourceMapper::new(&[])`. A Phase 3 `new` that rejects
    /// binaries without debug info has to keep an empty input from panicking, or update
    /// that call site.
    ///
    /// [`load_wasm_file`]: crate::tracer::load_wasm_file
    pub fn new(_wasm_bytes: &[u8]) -> Self {
        Self {}
    }

    /// Resolve one program counter to the source frame that produced it.
    ///
    /// Returns `None` whenever the address cannot be attributed to a source line — no DWARF,
    /// an address outside every range, or a pc that was never a real offset. It never
    /// panics and never returns a half-filled frame, because aggregation calls this for
    /// every event: a frame with an empty function name would add an anonymous root to the
    /// tree and silently absorb the cost of everything unattributed.
    ///
    /// Takes `&self`, so one mapper can serve a whole trace, and Issue 27's cache would make
    /// `&mut self` — a change to weigh against `aggregate` holding `&SourceMapper`.
    ///
    /// # Examples
    ///
    /// ```
    /// use soroban_cost_profiler::source_map::SourceMapper;
    ///
    /// // A binary with no debug info resolves nothing, and must not panic.
    /// let mapper = SourceMapper::new(&[]);
    /// assert!(mapper.resolve(0).is_none());
    /// ```
    pub fn resolve(&self, _pc: usize) -> Option<SourceFrame> {
        // TODO(Phase 3): translate the pc to a code-section offset (Issue 22), query the
        // addr2line context, demangle (Issue 23) and collapse closures (Issue 24).
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stage 2 is built from `SourceMapper::new(&[])` in `main`, and Phase 3 will build it
    /// from whatever the user pointed the profiler at — including a file that is not a
    /// module at all. Construction must not be the thing that crashes the run.
    #[test]
    fn construction_tolerates_bytes_without_debug_info() {
        let _empty = SourceMapper::new(&[]);
        let _truncated_header = SourceMapper::new(b"\0asm\x01\x00\x00\x00");
        let _not_wasm = SourceMapper::new(b"definitely not a wasm module");
    }

    /// The property aggregation relies on: an unattributable address is `None`, not a
    /// frame. If Phase 3 makes construction fallible, these calls gain an `unwrap`.
    #[test]
    fn an_unattributable_pc_yields_no_frame() {
        let mapper = SourceMapper::new(&[]);
        for pc in [0usize, 1, 64, 4096, usize::MAX] {
            assert!(
                mapper.resolve(pc).is_none(),
                "pc {pc} resolved to a frame from a binary that carries no DWARF"
            );
        }
    }
}
