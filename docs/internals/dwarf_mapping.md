# DWARF Mapping: how a WASM address becomes a Rust source frame

Stage 2 of the pipeline (`src/source_map.rs`) turns the program counters in a `TraceEvent` stream
into the names a flamegraph shows. This is the contributor-facing guide to what `addr2line` actually
does with those bytes, and to the ways it can return nothing while every call in it succeeded.

Where the numbers come from: they are measured against the committed fixtures in this repository, on
`rustc 1.98.1` / `wasm32-unknown-unknown`. `docs/spikes/02_wasm_name_section_fallback.md` records the
build-side half of the story; this document covers the lookup-side half.

## DWARF in a wasm file is a pile of custom sections

An ELF or Mach-O binary has well-known program headers for debug info. A `.wasm` file has one section
type — "custom" (`id 0`) — and debug info arrives as several of them, each named exactly as the DWARF
section is named:

```
.debug_abbrev   .debug_info   .debug_line   .debug_ranges   .debug_str
```

They are *not* merged into one `DWARF` blob, so reading them needs no container parser: `object` and
`memmap2` are not dependencies, and `addr2line` is built with `default-features = false`. Optional
sections may simply be absent depending on the DWARF version — a DWARF 4 build has no
`.debug_line_str`, `.debug_addr` or `.debug_str_offsets` — which `gimli` is told about by handing it
empty bytes for that section id.

`WasmSections::parse` walks the section table (section id, payload length, then, for a custom
section, the name's length prefix and bytes) and keeps `name` plus anything starting with `.debug`.
That is the only hand-written parsing in the stage. Everything above it is `gimli`:

```rust
let dwarf = gimli::Dwarf::load(|id| reader(sections.get(id.name()).unwrap_or(&[])))?;
let context = addr2line::Context::from_dwarf(dwarf)?;
let frame = context.find_frames(address).skip_all_loads()?.next()?;
```

The `AGENTS.md` constraint "no custom DWARF parsing" is satisfied by that boundary: we slice *wasm*
sections, never DWARF ones. Note `Dwarf::load` copies each section into an `Rc<[u8]>` reader rather
than borrowing the caller's buffer, because the mapper has to outlive the `&[u8]` it was constructed
from and keep serving lookups for a whole trace.

## The address space, which is the part that bites

DWARF line tables in a wasm build are written against **offsets into the code section's payload**:
the bytes after the section id and its length, so address `0` is the function-count byte and each
function's body begins one byte-size-prefix after its own size LEB. They are not linear-memory
addresses, not file offsets, and not the `pc` any engine reports.

The base is measured, not assumed. In `fixtures/dwarf_probe/dwarf_probe.wasm` the code section's
payload starts at file offset `111`, and framing its three functions gives bodies at code-relative
`2..16`, `18..157` and `158..165` — which is exactly where `resolve` begins answering, so
`CodeMap::bodies` (wasm framing) and `SourceMapper::resolve` (DWARF) agree only if the base is right.
The same arithmetic holds on the 622 KB `dummy-contract` build, which additionally has four function
**imports**: its payload begins at file offset `190` and the first defined body is address `2`, while
that function's index in the module's index space is `4`. Code-section order, function index and file
offset are three different numbers, and conflating any two of them is the bug class here.

`CodeMap` is that translation, built during the same section walk that finds the DWARF:
`to_code_address(file_offset)` moves a position in the file into DWARF's space, `function_at(address)`
says which body contains it, and `SourceMapper::resolve_file_offset` does the two in order. It is
best effort by design — a module whose function list overruns its section yields
`SourceMapper::code_map() == None` rather than an error, because a caller already holding a DWARF
address needs no map.

What still cannot be resolved is an address from the engine, and that is a property of `wasmi` 2.0
rather than of this stage: its only execution hook, `Store::call_hook`, passes the hook *variant*
and nothing else — no callee, no instruction hook, no program counter — so every event the tracer
records is written at `pc = 0` (see `docs/internals/tracer_architecture.md` and
`ExecutionTracer::invoke_function`'s docs). It is not merely unexposed: the engine re-encodes wasm
bytecode into its own variable-length instruction stream during translation and retains no table back
to the original offsets, so even a leaked instruction pointer would be an index into a different
program. The finest code-section granularity reachable at runtime is therefore a **function body**,
which is what `CodeMap::bodies` is for. Three consequences to keep in mind when you touch this:

* A mapping bug here is invisible in CI and visible in output. `resolve(0)` on a real binary
  probably returns `None` — offset `0` is the function-count byte, before the first function's line
  program — so a broken translation shows up as an empty flamegraph, not a failing test.
* Never treat "resolved to nothing" as "is not an instruction". Addresses `0`, `1`, `16` and `17` of
  the fixture are inside the code section and belong to no function; `CodeMap::function_at` separates
  those two answers, which #162's degenerate-mapping warning depends on.
* `addr2line` probes the half-open range `[address, address + 1)`, so `u64::MAX` overflows inside the
  dependency and panics a debug build. `resolve` rejects that value before the lookup instead; no
  code section is within orders of magnitude of it.

## What one lookup can and cannot tell you

`Context::find_frames` returns an iterator of `Frame`s, each carrying both a `location` (file, line,
column) and a `function` (the symbol, plus its DWARF language). One call therefore answers file, line
and name — which is why `resolve` does not also call `find_location`.

Sweeping every byte offset of `fixtures/dwarf_probe/dwarf_probe.wasm`'s 165-byte code section gives
160 frames and 133 lines. The six misses (`0`, `1`, `16`, `17`, `157`, `165`) are gaps between
function ranges, and the hits are not uniform either:

| address | function name | file | line | why |
| --- | --- | --- | --- | --- |
| `0`, `1` | — | — | — | before the first function's range |
| `2` | `caller_of_heavy` | *none* | *none* | the prologue precedes the first line-program entry |
| `3`–`13` | `caller_of_heavy` | `…/src/lib.rs` | `39` | the call on the function's last line |
| `14` | `<u64>::wrapping_add` | `…/library/core/src/num/uint_macros.rs` | `2612` | inlined core code, innermost frame wins |
| `15` | `caller_of_heavy` | `…/src/lib.rs` | `40` | back to the caller after that inlined copy |
| `16`, `17` | — | — | — | gap between function ranges |
| `61`–`71`, `75`–`89` | `memory_heavy_loop` | `…/src/lib.rs` | *none* | 26 addresses with a file and no line |
| `158` | `compute_heavy_loop` | `…/src/lib.rs` | `10` | attributes to the signature line |
| `165` | — | — | — | one past the last function's range |

`column` behaves the same way and is deliberately not in `SourceFrame` yet: the same sweep returns
`Some(39)` at `pc = 3`, `Some(13)` at `14`, and `Some(0)` at `158` — including a `0`, so a column is
another value that must stay an `Option` if it is ever surfaced.

So `SourceFrame`'s three fields have three different contracts, and this is deliberate:

* `function_name: String` is **required**. `CallStackNode`'s children are keyed by function name, so
  an unnamed frame would pool every unattributable address into one anonymous root and quietly absorb
  their cost. `resolve` returns `None` rather than build one.
* `file_path` and `line_number` are **independent** `Option`s. A stripped build can still yield a
  name; a line table can still yield a file with no line. Never fold a missing line into `Some(0)` —
  line 0 is a real value in some DWARF, and `Option` is what distinguishes "absent" from "line zero".

## Inlined frames

`find_frames` yields innermost-first: for address `14` the first frame is `<u64>::wrapping_add` in
core and the second is `caller_of_heavy`, whose source line actually contains the call. `resolve`
takes the first, because at a leaf address the inlined function is the thing that executed — the
distinction that makes a flamegraph actionable rather than merely correct-looking.

The whole stack is what a real flamegraph wants, and that is #156: change `resolve` to return
`Vec<SourceFrame>` and have the aggregator push each entry as a stack level. Nothing about the
lookup changes; only how many `next()` calls you make.

## Demangling

`FunctionName::demangle()` routes `DW_LANG_Rust` through `rustc-demangle` and formats with `{:#}`, the
alternate form that drops the crate-disambiguator hashes:

```
_RNvMs7_NtCsknUcikIyyBm_4core3numy12wrapping_add   (raw, from the fixture at address 14)
<u64>::wrapping_add                               (shown)
```

`rustc-demangle` is enabled in `Cargo.toml` specifically for this; `cpp_demangle` is not, and C++
symbols therefore pass through undemangled. When the language is absent or the name does not parse,
`demangle()` returns the raw symbol unchanged — which is why the mangled forms here come only from
inlined dependencies. All three fixture contract functions are `#[no_mangle] extern "C"`, so
`compute_heavy_loop` arrives as that plain C symbol, while the one `_R…` in the sweep is core's. That
makes a bare `_R` prefix reaching a frame a real bug rather than an expected shape, and there is a test
sweeping the whole fixture code section for exactly that.

## The `name` section is a different animal

`name` is a wasm custom section that maps function-index-space indices to symbol names. It is present
even when no DWARF is (`debug = false` builds carry it), it needs no `gimli`, and it gives function
names and nothing else — never a file, never a line. The full layout, its version-less framing quirk,
and the fact that binaryen deletes it unless `-g` is passed are in
`docs/spikes/02_wasm_name_section_fallback.md`. Reading it is #157; `retain()` already keeps the
bytes.

## Producing a binary this stage can map

```toml
[profile.release.package.your_contract]
debug = "line-tables-only"   # or `debug = 1` for inlined-function names too
```

and then profile `target/wasm32-unknown-unknown/release/<contract>.wasm`. If you run `wasm-opt`
yourself, pass `-g`; if you use `stellar contract build`, its optimize step passes no `-g` and the
deployed artifact cannot be mapped by either source. The trap to remember: an optimized binary still
*loads* — its `.debug_*` sections survive, `has_debug_info()` returns `true`, and every lookup returns
`None`, because the line table describes code the optimizer rewrote. `MissingDebugInfo` cannot fire
there, which is what #162's warning is for.

Costs, measured: `debug = "line-tables-only"` took the fixture contract from 3.1 KB to 622,507 bytes,
619 KB of which is the five `.debug_*` sections against a 488-byte code section. A `#![no_std]`,
`panic = "abort"` build shows the floor: the same three functions cost 1,690 bytes with DWARF
(`fixtures/dwarf_probe/dwarf_probe.wasm`) and 595 without (`dwarf_probe_no_debug.wasm`), which is why
the pair is committed and the big one is not.

## When a mapping resolves nothing

1. `has_debug_info()` — if `false`, read the `SourceMapError`: `MissingDebugInfo` names the sections
   that *were* present, so you can see whether there is a `name` section to fall back to.
2. Confirm the address space. `code_map()` gives the section's extent and each body's range:
   `to_code_address` returns `None` for an offset that is not in the code section at all, and
   `function_at` returns `None` for one that is in it but belongs to no function — a linear-memory or
   file offset will resolve nothing while looking perfectly plausible.
3. Sweep, don't sample. Iterate `0..code_size` calling `find_location` and count hits — that is how
   the 160/166 figure was produced, and how the "loads but resolves 0" case was distinguished from a
   missing-section case.
4. Look for an optimizer step between the build and the profile run.
5. Remember paths are the builder's: DWARF stores `DW_AT_comp_dir`, so a committed fixture carries the
   absolute path of the machine that built it, and tests should assert `ends_with` rather than equal.

## See also

* `src/source_map.rs` — the module docs carry the same facts at code level, plus the dependency
  rationale for `gimli`'s feature set.
* `docs/spikes/02_wasm_name_section_fallback.md` — what survives `wasm-opt` and `stellar contract
  build`, and the `name`-section layout.
* `docs/internals/tracer_architecture.md` — why the `pc` reaching this stage is currently always `0`.
* `fixtures/dwarf_probe/README.md` and `build.sh` — how the committed fixtures are made.
