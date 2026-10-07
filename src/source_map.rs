use crate::models::SourceFrame;
use addr2line::FunctionName;
use std::rc::Rc;

/// The reader type `addr2line` is instantiated with.
///
/// An `Rc`-backed slice rather than `EndianSlice<'a, ..>`: the mapper has to own the section
/// bytes, because the bytes it symbolizes against are read once from the binary and the
/// `Context` then serves a whole trace. A borrowed reader would tie the mapper's lifetime to
/// the buffer the caller passed to [`SourceMapper::new`].
type Reader = gimli::EndianRcSlice<gimli::NativeEndian>;

type Dwarf = gimli::Dwarf<Reader>;
type Context = addr2line::Context<Reader>;

/// Why a binary could not be symbolized, in the user's terms.
///
/// Every variant's message names the flag or the file that fixes it: this error is what a
/// contract author reads when the flamegraph comes out unnamed, and "invalid DWARF" alone
/// tells them nothing actionable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMapError {
    /// The bytes are not a WASM module — too short to hold a header, or the wrong magic.
    NotWasm,
    /// A section header claimed more bytes than the file contains, i.e. a truncated download
    /// or a partial write rather than a malformed build.
    Truncated,
    /// A valid module with no `.debug_info` section, which is the ordinary case for a release
    /// build: `debug` is off for `release` by default, so nothing was emitted to map.
    MissingDebugInfo {
        /// Custom sections that *were* present, so the message can point out a `name` section
        /// worth falling back to (see the function-name-only path in Phase 3's fallback chain).
        custom_sections: Vec<String>,
    },
    /// DWARF sections are present but `gimli` could not read them — an incomplete build, an
    /// unsupported DWARF version, or a section that was stripped after linking.
    UnreadableDwarf { reason: String },
}

impl std::fmt::Display for SourceMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotWasm => write!(
                f,
                "not a WebAssembly module: expected the 8-byte header `\\0asm\\x01\\0\\0\\0`. \
                 Pass the `.wasm` file itself, not a `.wat` text file or an archive."
            ),
            Self::Truncated => write!(
                f,
                "WebAssembly module ended in the middle of a section. Re-download or rebuild it: \
                 a truncated file cannot be mapped, and would fail to instantiate too."
            ),
            Self::MissingDebugInfo { custom_sections } => {
                write!(
                    f,
                    "no `.debug_info` section, so program counters cannot be mapped to Rust \
                     source lines. Build the contract with debug info enabled — \
                     `[profile.release] debug = \"line-tables-only\"` is enough for `file:line` \
                     frames — and profile that artifact rather than the stripped one \
                     (`wasm-opt`, and `stellar contract build`, strip debug info)."
                )?;
                if custom_sections.is_empty() {
                    write!(f, " The module carries no custom sections at all.")?;
                } else {
                    write!(f, " Sections present: {}.", custom_sections.join(", "))?;
                }
                Ok(())
            }
            Self::UnreadableDwarf { reason } => write!(
                f,
                "the binary has DWARF sections but they could not be read ({reason}). \
                 This usually means the artifact was partially stripped or built with an \
                 unsupported DWARF version; rebuild it from source."
            ),
        }
    }
}

impl std::error::Error for SourceMapError {}

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
/// # What it holds
///
/// An `addr2line::Context` built from the DWARF custom sections of the binary that was loaded
/// ([`SourceMapper::new`]), or nothing at all ([`SourceMapper::unmapped`]) for a caller that
/// chose to continue without symbols. Construction is fallible and reports *why* there are no
/// symbols, because "the binary has no debug info" and "the address is outside every range"
/// need different fixes and only the first is the user's to make.
///
/// The facts below were checked against real builds rather than assumed, because each one
/// decided how much code Phase 3 is:
///
/// * **DWARF arrives as separate WASM custom sections, so loading is a section walk.** A
///   `wasm32-unknown-unknown` build with debug info carries `.debug_abbrev`, `.debug_info`,
///   `.debug_str`, `.debug_line`, `.debug_ranges` (and `.debug_loc` where applicable) as
///   individual custom sections, alongside `name`, `producers`, `target_features` and
///   Soroban's `contractspecv0`. They are not merged into one `DWARF` section, so
///   `ROADMAP.md`'s "parse the `.debug_info` and `.debug_line` sections" is literally right,
///   and the only hand-written parsing here is the WASM section table — never DWARF itself.
///   `AGENTS.md`'s "no custom DWARF parsing" rule holds: `gimli::Dwarf::load` reads the
///   sections, `addr2line::Context::from_dwarf` builds the index.
/// * **`addr2line` needs no file wrapper and adds no dependency weight.** Its
///   `default-features = false, features = ["std", "rustc-demangle"]` build pulls neither
///   `object` nor `cpp_demangle` nor `memmap2`, and both it and `gimli` were already in
///   `Cargo.lock` via `backtrace`.
/// * **`gimli` has to be a direct dependency anyway.** `addr2line` exposes `gimli` only as a
///   re-export of its own instantiation, and that build enables `features = ["read"]`, which
///   omits `endian-reader` — so `addr2line::gimli::EndianRcSlice`, the owned reader a mapper
///   that outlives the caller's buffer needs, does not exist through it. Naming gimli in
///   `Cargo.toml` adds `endian-reader` and `std` to the same 0.32.3 already resolved, and `std`
///   is not optional here: `EndianRcSlice` only implements `gimli::Reader` once
///   `stable_deref_trait`'s `std` feature gives `Rc<[u8]>: CloneStableDeref`. The whole footprint
///   is those two small pure-Rust crates entering `Cargo.lock`.
/// * **Debug info costs more than the contract.** The same fixture is 3.1 KB stripped and 622 KB
///   with `debug = "line-tables-only"` — 619 KB of which is DWARF custom sections against a
///   488-byte code section — because a `std`-linked build emits debug info for every inlined
///   dependency, not just the contract's own functions. That is the price of profiling a real
///   Soroban build, and it is why `fixtures/build.sh`'s output is a *profiling input*, never
///   something to deploy. A `#![no_std]` crate with `panic = "abort"` shows the floor: the
///   same three functions cost 1.7 KB total.
/// * **DWARF addresses are code-section-relative, and `wasmi` never hands us one.** Looking up
///   `4`, `8` and `16` in that build returned `src/lib.rs:2` (the function's signature) and
///   `64` returned `src/lib.rs:5` (its loop body) — offsets into the code section, matching
///   #153's translation note. [`SourceMapper::resolve`] now turns such an offset into a frame:
///   sweeping the committed fixture's code section resolves 160 of its 166 addresses, 133 of them
///   to a line. What is still missing is an address worth resolving, because `wasmi` 2.0 gives a
///   call hook no program counter and offers no instruction hook at all, so every event
///   `invoke_function` records is written at `pc = 0`. See [`invoke_function`] for what the hooks
///   can and cannot see.
/// * **The `name` section survives this build path**, so a function-name-only fallback is real
///   (#157); a plain `cargo build` does not strip it, while
///   `stellar contract build` does. That is the precedence the docs should describe: DWARF ->
///   `name` -> function index.
///
/// Two traps the resolution issues will hit, both seen in the probe that produced the numbers
/// above:
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
/// [`SourceMapper::new`]: SourceMapper::new
/// [`SourceMapper::unmapped`]: SourceMapper::unmapped
/// [`SourceMapper::resolve`]: SourceMapper::resolve
pub struct SourceMapper {
    context: Option<Context>,
}

impl SourceMapper {
    /// Build a mapper for one already-loaded WASM binary, reading its DWARF.
    ///
    /// Takes bytes rather than a path because [`load_wasm_file`] has already read and validated
    /// the file, and the same bytes are handed to `parse_module` — reading twice would let the
    /// traced binary and the symbolized binary disagree.
    ///
    /// The `Context` is built here rather than on first lookup: it walks every compilation unit,
    /// which is expensive, and a trace then reads it millions of times. A caller that cannot
    /// afford to fail on a binary without symbols should match on [`SourceMapError`] and fall
    /// back to [`SourceMapper::unmapped`] — the profiler's job is to keep running and say why
    /// frames are unnamed, not to abort the run being measured.
    ///
    /// # Errors
    ///
    /// Returns [`SourceMapError`] when the bytes are not a module ([`SourceMapError::NotWasm`],
    /// [`SourceMapError::Truncated`]) or carry no usable DWARF
    /// ([`SourceMapError::MissingDebugInfo`], [`SourceMapError::UnreadableDwarf`]). Each message
    /// names the flag or step that would fix it.
    ///
    /// # Examples
    ///
    /// ```
    /// use soroban_cost_profiler::source_map::{SourceMapper, SourceMapError};
    ///
    /// // A release build with debug info off is the ordinary case, and it is an error worth
    /// // reporting: the flamegraph will be unnamed, and only the user can fix the build.
    /// let stripped = include_bytes!("../fixtures/dwarf_probe/dwarf_probe_no_debug.wasm");
    /// let error = SourceMapper::new(stripped).err().expect("this fixture ships without DWARF");
    /// assert!(matches!(error, SourceMapError::MissingDebugInfo { .. }));
    ///
    /// // The same functions built with `debug = 1` load, and the difference is the point.
    /// let mapped = SourceMapper::new(include_bytes!("../fixtures/dwarf_probe/dwarf_probe.wasm"));
    /// assert!(mapped.unwrap().has_debug_info());
    /// ```
    ///
    /// [`load_wasm_file`]: crate::tracer::load_wasm_file
    pub fn new(wasm_bytes: &[u8]) -> Result<Self, SourceMapError> {
        let sections = WasmSections::parse(wasm_bytes)?;

        // `.debug_info` is what makes the other sections meaningful; a module that has line
        // tables but no compilation units cannot yield a function name.
        if sections.get(DEBUG_INFO).is_none() {
            return Err(SourceMapError::MissingDebugInfo {
                custom_sections: sections.custom_names(),
            });
        }

        let context = Context::from_dwarf(load_dwarf(&sections)).map_err(|error| {
            SourceMapError::UnreadableDwarf {
                reason: error.to_string(),
            }
        })?;

        Ok(Self {
            context: Some(context),
        })
    }

    /// A mapper that resolves nothing, for a run that continues without symbols.
    ///
    /// This is the degraded-but-working path: Stages 3 and 4 still produce a tree, keyed by the
    /// `wasm[pc]` / `host[pc]` names the tracer already has. `main`'s harness uses it because it
    /// has no binary to read yet — Phase 5's CLI replaces it with [`SourceMapper::new`] plus the
    /// warning the returned error carries.
    pub fn unmapped() -> Self {
        Self { context: None }
    }

    /// Whether this mapper has DWARF to resolve against.
    ///
    /// Lets a caller distinguish "no frames because nothing was attributed" from "no frames
    /// because no symbols were loaded" — the difference between a bug in the profiler and a
    /// user's missing build flag, which the CLI has to report.
    pub fn has_debug_info(&self) -> bool {
        self.context.is_some()
    }

    /// Resolve one program counter to the source frame that produced it.
    ///
    /// `pc` is an offset into the WASM **code section**, the address space the DWARF line tables
    /// are written against; #153 owns translating whatever the engine reports into that form.
    ///
    /// Returns `None` when the address has no function to name: no DWARF loaded, an address
    /// outside every range, one too large to be a code-section offset, a lookup that `gimli`
    /// could not complete, or a frame whose name is empty. The name is what makes a frame, because `CallStackNode`'s children are keyed by
    /// `function_name` — an unnamed frame would pool every unattributable address into one
    /// anonymous root and quietly absorb their cost. The location fields stay optional and
    /// independent: measured against `fixtures/dwarf_probe`, code-section address `2` yields a
    /// name and no location at all (the prologue precedes the first line program), and `61`..`71`
    /// yield a file with no line.
    ///
    /// The frame is the **innermost** one at that address, which for inlined code is the inlined
    /// function rather than its caller: address `14` resolves to `<u64>::wrapping_add` inside
    /// `caller_of_heavy`. Keeping the whole inline stack is #156.
    ///
    /// Takes `&self`, so one mapper can serve a whole trace, and #158's cache would make it
    /// `&mut self` — a change to weigh against `aggregate` holding `&SourceMapper`.
    ///
    /// # Examples
    ///
    /// ```
    /// use soroban_cost_profiler::source_map::SourceMapper;
    ///
    /// // A mapper without symbols resolves nothing, and must not panic.
    /// let mapper = SourceMapper::unmapped();
    /// assert!(mapper.resolve(0).is_none());
    ///
    /// // A real build resolves: this fixture is Rust code compiled for wasm32-unknown-unknown.
    /// let fixture = include_bytes!("../fixtures/dwarf_probe/dwarf_probe.wasm");
    /// let mapper = SourceMapper::new(fixture).expect("the fixture carries DWARF");
    /// let frame = mapper.resolve(3).expect("address 3 is inside `caller_of_heavy`");
    /// assert_eq!(frame.function_name, "caller_of_heavy");
    /// assert_eq!(frame.line_number, Some(39));
    /// ```
    pub fn resolve(&self, pc: usize) -> Option<SourceFrame> {
        let context = self.context.as_ref()?;

        // `addr2line` probes the half-open range `[address, address + 1)`, so `u64::MAX` overflows
        // inside that computation and panics a debug build. No code section is within orders of
        // magnitude of that, so an address this high is not an offset and gets the same answer as
        // any other address outside every range.
        let address = u64::try_from(pc).ok()?;
        if address == u64::MAX {
            return None;
        }

        // `skip_all_loads`: every section was copied into the reader at construction, so there is
        // nothing to load, and a split-DWARF request would try to open a file that never existed.
        let mut frames = context.find_frames(address).skip_all_loads().ok()?;
        let frame = frames.next().ok()??;
        let function_name = frame_name(frame.function.as_ref()?)?;

        let location = frame.location.as_ref();
        Some(SourceFrame {
            function_name,
            file_path: location.and_then(|loc| loc.file).map(str::to_string),
            line_number: location.and_then(|loc| loc.line),
        })
    }
}

/// The section whose absence means the binary cannot be source-mapped at all.
///
/// Line tables alone cannot name a function, and an address only resolves through a compilation
/// unit, so `.debug_info` is what makes the other `.debug_*` sections meaningful.
const DEBUG_INFO: &str = ".debug_info";

/// DWARF section names, as they appear in a WASM custom section.
///
/// Rust emits each section as its own custom section rather than one merged `DWARF` blob, so
/// [`WasmSections::parse`] can hand gimli exactly the bytes [`gimli::SectionId::name`] asks for.
/// A lookup miss returns empty bytes, which is how gimli is told "this optional section does not
/// exist" — DWARF 4 builds have no `.debug_line_str`, `.debug_addr` or `.debug_str_offsets`.
///
/// Infallible by construction: `Dwarf::load` only fails through the closure, and copying bytes
/// into an owned reader cannot fail. What *can* fail is reading the contents, which is
/// [`addr2line::Context::from_dwarf`]'s error and the caller's to report.
fn load_dwarf(sections: &WasmSections) -> Dwarf {
    // Copied per section, once, at load: the mapper outlives the caller's buffer, so the bytes
    // it indexes have to be owned.
    let reader =
        |bytes: &[u8]| Reader::new(Rc::from(bytes.to_vec()), gimli::NativeEndian::default());

    Dwarf::load(&mut |id: gimli::SectionId| {
        Ok::<_, std::convert::Infallible>(reader(sections.get(id.name()).unwrap_or(&[])))
    })
    .expect("copying section bytes into a reader cannot fail")
}

/// The custom sections a mapper keeps, in file order.
struct WasmSections {
    /// `(name, payload)`, holding only the sections this stage can use: `.debug_*` for source
    /// mapping and `name` for the function-name-only fallback. Everything else — `producers`,
    /// `target_features`, Soroban's `contractspecv0` — would be a copy of bytes nothing reads.
    retained: Vec<(String, Vec<u8>)>,
}

impl WasmSections {
    /// Walk the WASM section table, keeping the sections Stage 2 reads.
    ///
    /// A hand-written parser for the *container* only: section id, payload length, and for
    /// custom sections the name prefix. That is the whole module structure `addr2line` needs and
    /// it never inspects DWARF itself, which is what keeps this inside `AGENTS.md`'s "no custom
    /// DWARF parsing" rule. Deliberately strict about truncation and lenient about everything
    /// else: a module with sections this stage ignores still maps fine.
    fn parse(bytes: &[u8]) -> Result<Self, SourceMapError> {
        if bytes.len() < 8 || &bytes[..4] != b"\0asm" {
            return Err(SourceMapError::NotWasm);
        }

        let mut cursor = 8; // past magic and version
        let mut retained = Vec::new();

        while cursor < bytes.len() {
            let id = Self::read_uleb(bytes, &mut cursor)?;
            let size = usize::try_from(Self::read_uleb(bytes, &mut cursor)?)
                .map_err(|_| SourceMapError::Truncated)?;
            let end = cursor.checked_add(size).ok_or(SourceMapError::Truncated)?;
            if end > bytes.len() {
                return Err(SourceMapError::Truncated);
            }

            // Section id 0 is a custom section: its payload is a name then the data.
            if id == 0 {
                let name_len = usize::try_from(Self::read_uleb(bytes, &mut cursor)?)
                    .map_err(|_| SourceMapError::Truncated)?;
                let name_start = cursor;
                let name_end = cursor
                    .checked_add(name_len)
                    .ok_or(SourceMapError::Truncated)?;
                if name_end > end {
                    return Err(SourceMapError::Truncated);
                }
                let name = std::str::from_utf8(&bytes[name_start..name_end])
                    .map_err(|_| SourceMapError::Truncated)?
                    .to_string();

                if retain(&name) {
                    // The section's own data starts after the name bytes, not after the length.
                    retained.push((name, bytes[name_end..end].to_vec()));
                }
            }

            cursor = end;
        }

        Ok(Self { retained })
    }

    /// Read a LEB128 unsigned integer, advancing the cursor.
    ///
    /// Rejects a byte that would overflow `u64` and a run that ends at the file's
    /// [`SourceMapError::Truncated`] rather than reading past the end or wrapping silently.
    fn read_uleb(bytes: &[u8], cursor: &mut usize) -> Result<u64, SourceMapError> {
        let mut value: u64 = 0;
        let mut shift = 0;

        loop {
            let byte = *bytes.get(*cursor).ok_or(SourceMapError::Truncated)?;
            *cursor += 1;

            if shift >= 64 || (byte & 0x7f) as u64 > u64::MAX >> shift {
                return Err(SourceMapError::Truncated);
            }
            value |= u64::from(byte & 0x7f) << shift;

            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn get(&self, name: &str) -> Option<&[u8]> {
        self.retained
            .iter()
            .find(|(section, _)| section == name)
            .map(|(_, bytes)| bytes.as_slice())
    }

    /// Names of the custom sections that were kept, for the error message that tells a user what
    /// their build actually contains.
    ///
    /// Only retained sections appear: the point of the list is "no `.debug_*` here, but there is
    /// a `name` section to fall back to", and `producers`/`target_features` would be noise.
    fn custom_names(&self) -> Vec<String> {
        self.retained.iter().map(|(name, _)| name.clone()).collect()
    }
}

/// The name to put in a frame, demangled where the DWARF says how.
///
/// `addr2line`'s `demangle()` applies `rustc-demangle` for `DW_LANG_Rust` and uses its *alternate*
/// format, which drops the `-<hash>` suffixes crate symbols carry — the difference between
/// `_RNvNtNtCs..17soroban_env_guest5guest3vec13vec_push_back` and a readable path. When the
/// language is absent or the name will not parse, it hands back the raw symbol unchanged, which is
/// why `#[no_mangle] extern "C"` functions and C symbols arrive as plain names here.
///
/// `None` means "no name to build a frame from", which [`SourceMapper::resolve`] treats as
/// unattributable; an empty string would key every such address to the same flamegraph frame.
fn frame_name(function: &FunctionName<Reader>) -> Option<String> {
    let name = function.demangle().ok()?;
    (!name.is_empty()).then(|| name.into_owned())
}

/// Whether a custom section is worth keeping in memory.
fn retain(name: &str) -> bool {
    name == "name" || name.starts_with(".debug")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode one LEB128 unsigned integer.
    fn uleb(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            out.push(if value == 0 { byte } else { byte | 0x80 });
            if value == 0 {
                return out;
            }
        }
    }

    /// Build a module carrying the given custom sections, so a test can describe a binary
    /// exactly rather than depending on whatever a toolchain happened to emit.
    fn module(custom: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();

        for (name, payload) in custom {
            let name_bytes = name.as_bytes();
            let mut section = uleb(name_bytes.len() as u64);
            section.extend_from_slice(name_bytes);
            section.extend_from_slice(payload);

            bytes.push(0); // custom section id
            bytes.extend(uleb(section.len() as u64));
            bytes.extend(section);
        }

        // One real code section, so the module is not nothing: `488` is the size of the fixture's.
        bytes.push(10);
        bytes.extend(uleb(2));
        bytes.extend([0x01, 0x00]);
        bytes
    }

    /// The fixture that carries DWARF, built by `fixtures/dwarf_probe/build.sh`.
    const DWARF_PROBE: &[u8] = include_bytes!("../fixtures/dwarf_probe/dwarf_probe.wasm");

    #[test]
    fn an_empty_input_is_not_a_module() {
        // #150: the ask is that `&[]` fails with the expected error rather than panicking or
        // silently yielding a mapper that resolves nothing.
        let error = SourceMapper::new(&[]).err().expect("no bytes, no module");

        assert_eq!(error, SourceMapError::NotWasm);
        assert!(
            error.to_string().contains("not a WebAssembly module"),
            "the message has to tell the user what to pass instead: {error}"
        );
    }

    #[test]
    fn bytes_that_are_not_wasm_are_rejected_by_magic() {
        for input in [
            b"definitely not a wasm module".as_slice(),
            b"WAT!(module)".as_slice(),
            b"\x7fELF\x02\x01\x01\x00".as_slice(),
        ] {
            assert_eq!(
                SourceMapper::new(input).err(),
                Some(SourceMapError::NotWasm),
                "a `{}`-byte input is not a module",
                input.len()
            );
        }
    }

    #[test]
    fn a_module_without_debug_info_reports_the_build_flag_that_fixes_it() {
        // The ordinary case for a release build, and the one a user can actually act on.
        let stripped = module(&[("name", b"functions"), ("producers", b"CL 17")]);

        let error = SourceMapper::new(&stripped)
            .err()
            .expect("a binary with no DWARF must not look like a successful load");

        let message = error.to_string();
        let SourceMapError::MissingDebugInfo { custom_sections } = &error else {
            panic!("expected the missing-DWARF error, got {error:?}");
        };
        assert!(
            message.contains("line-tables-only"),
            "the message has to name the setting that emits debug info: {message}"
        );
        assert!(
            message.contains(&custom_sections.join(", ")),
            "listing what *is* there is how a user notices a `name` section to fall back to: \
             {message}"
        );
    }

    #[test]
    fn truncated_section_headers_error_instead_of_reading_past_the_end() {
        // A section claims more bytes than the file holds — a partial download, not a bad build.
        let claims_too_much = [
            b"\0asm\x01\0\0\0\x00\x7f".to_vec(),
            b"\0asm\x01\0\0\0\x00\x04\x0b.debu".to_vec(),
            b"\0asm\x01\0\0\0\x00\x05\x0b\x00.debug_info".to_vec(),
        ];

        for input in claims_too_much {
            assert_eq!(
                SourceMapper::new(&input).err(),
                Some(SourceMapError::Truncated),
                "input {:?} must be reported as truncated, not panicking",
                String::from_utf8_lossy(&input)
            );
        }
    }

    #[test]
    fn sections_this_stage_ignores_are_skipped_without_error() {
        // Soroban emits `contractspecv0`/`contractmetav0` and LLVM `producers`/`target_features`;
        // a module carrying those plus no DWARF is a normal binary, so the only complaint is the
        // missing debug info — and the retained names show which sections were noticed.
        let with_spec = module(&[
            ("contractspecv0", b"\x01\x02\x03"),
            (".debug_abbrev", b"\x01"),
            ("target_features", b"\x00"),
        ]);

        let error = SourceMapper::new(&with_spec)
            .err()
            .expect("no .debug_info present");

        match error {
            SourceMapError::MissingDebugInfo { custom_sections } => assert_eq!(
                custom_sections,
                vec![".debug_abbrev".to_string()],
                "only DWARF and `name` are worth keeping a copy of"
            ),
            other => panic!("expected the missing-DWARF error, got {other:?}"),
        }
    }

    #[test]
    fn a_real_build_with_line_tables_loads() {
        // The happy path against DWARF a toolchain actually emitted, not a fixture assembled by
        // hand: this is the artifact `fixtures/dwarf_probe/build.sh` produces from Rust source.
        let mapper = SourceMapper::new(DWARF_PROBE)
            .unwrap_or_else(|error| panic!("the DWARF-bearing fixture should load: {error}"));

        assert!(mapper.has_debug_info());

        // Present is not the same as usable. Walking every compilation unit's line table is the
        // check that the section bytes reached gimli whole: a mis-sliced custom section still
        // loads, then fails here with an unexpected end of input.
        let context = mapper.context.as_ref().expect("has_debug_info()");
        context.parse_lines().unwrap_or_else(|error| {
            panic!("the fixture's line tables should be readable: {error}")
        });
    }

    #[test]
    fn unreadable_dwarf_is_reported_rather_than_panicking() {
        // Present-but-nonsense sections: `gimli` rejects them, and Phase 5's CLI needs that to be
        // a message about rebuilding, not a crash in the middle of a profile run.
        let garbage = module(&[
            (".debug_info", b"not dwarf at all"),
            (".debug_abbrev", b"also not dwarf"),
            (".debug_line", b"nor this"),
        ]);

        let error = SourceMapper::new(&garbage)
            .err()
            .expect("malformed DWARF must not load as if it were fine");

        let SourceMapError::UnreadableDwarf { ref reason } = error else {
            panic!("expected the unreadable-DWARF error, got {error:?}");
        };
        assert!(
            error.to_string().contains("rebuild"),
            "the message has to point at a fix: {error}"
        );
        assert!(!reason.is_empty(), "and say what gimli objected to");
    }

    /// The frame the fixture's DWARF gives for `pc`, with a failure that names the address.
    fn frame_at(mapper: &SourceMapper, pc: usize) -> SourceFrame {
        mapper
            .resolve(pc)
            .unwrap_or_else(|| panic!("pc {pc} lies inside the fixture's code section"))
    }

    #[test]
    fn a_code_offset_resolves_to_its_function_file_and_line() {
        // One address from the middle of each of the fixture's three functions. `pc` is
        // code-section-relative, which is the form #153 hands to this call. The line is checked
        // against the function's source span rather than a single number so that rebuilding the
        // fixture with a different toolchain cannot fail the test for the wrong reason.
        let mapper = SourceMapper::new(DWARF_PROBE).expect("the fixture carries DWARF");

        for (pc, function, span) in [
            (3usize, "caller_of_heavy", 38..=40u32),
            (20, "memory_heavy_loop", 21..=35),
            (158, "compute_heavy_loop", 10..=18),
        ] {
            let frame = frame_at(&mapper, pc);

            assert_eq!(
                frame.function_name, function,
                "pc {pc} named the wrong function"
            );
            assert!(
                frame
                    .file_path
                    .as_deref()
                    .is_some_and(|file| file.ends_with("fixtures/dwarf_probe/src/lib.rs")),
                "pc {pc} resolved to {:?}, not the fixture's only source file",
                frame.file_path
            );
            let line = frame
                .line_number
                .unwrap_or_else(|| panic!("pc {pc} in `{function}` should carry a line number"));
            assert!(
                span.contains(&line),
                "pc {pc} resolved to `{function}:{line}`, outside its span {span:?}"
            );
        }
    }

    #[test]
    fn a_prologue_resolves_a_function_name_with_no_location() {
        // The first instructions of `caller_of_heavy` precede the line program's first entry, so
        // DWARF knows the function but not the line. Cost still has to land somewhere, so a name
        // is enough to make a frame, and the two location fields stay `None` rather than becoming
        // line 0. (`61`..`71` are the mirror case: a file and no line.)
        let mapper = SourceMapper::new(DWARF_PROBE).expect("the fixture carries DWARF");

        let frame = frame_at(&mapper, 2);

        assert_eq!(frame.function_name, "caller_of_heavy");
        assert_eq!(frame.file_path, None);
        assert_eq!(frame.line_number, None);
    }

    #[test]
    fn a_frame_in_inlined_dependency_code_is_demangled() {
        // `caller_of_heavy`'s body inlines `u64::wrapping_add`, so the innermost frame at `14` is
        // core code reached through a Rust v0 symbol. This is the whole of #148's example
        // (`my_contract::swap` is the same kind of name) and it shows the frame pointing at a
        // registry path next to the contract's own -- the reason anything that groups frames by
        // file has to expect both.
        let mapper = SourceMapper::new(DWARF_PROBE).expect("the fixture carries DWARF");

        let frame = frame_at(&mapper, 14);

        assert_eq!(frame.function_name, "<u64>::wrapping_add");
        assert!(
            frame
                .file_path
                .as_deref()
                .is_some_and(|file| file.ends_with("core/src/num/uint_macros.rs")),
            "expected inlined core code, got {:?}",
            frame.file_path
        );
        assert!(frame.line_number.is_some_and(|line| line > 0));
    }

    #[test]
    fn resolution_covers_most_of_the_code_section() {
        // Not "DWARF is present" but "DWARF maps an executed address": the fixture's code section
        // is 165 bytes, and one address per byte is swept. 160 of those 166 resolve; the rest are
        // the two-byte gaps between functions and the address past the end.
        let mapper = SourceMapper::new(DWARF_PROBE).expect("the fixture carries DWARF");

        let frames: Vec<SourceFrame> = (0..166usize).filter_map(|pc| mapper.resolve(pc)).collect();
        let names: Vec<&str> = frames
            .iter()
            .map(|frame| frame.function_name.as_str())
            .collect();

        assert!(
            frames.len() > 150,
            "only {} of 166 code-section offsets resolved",
            frames.len()
        );
        for function in ["caller_of_heavy", "memory_heavy_loop", "compute_heavy_loop"] {
            assert!(
                names.contains(&function),
                "`{function}` was never named; frames resolved as {names:?}"
            );
        }
        for frame in &frames {
            assert!(!frame.function_name.is_empty(), "an empty frame: {frame:?}");
            assert!(
                !frame.function_name.starts_with("_R"),
                "a mangled symbol reached a frame: {:?}",
                frame.function_name
            );
        }
    }

    #[test]
    fn addresses_outside_the_code_section_resolve_to_nothing() {
        // The gaps between functions (`16`, `17`, `157`), the first address past the end (`166`),
        // and the values a mis-translated or uninitialised pc looks like. `usize::MAX` is the
        // interesting one: `addr2line` probes `[address, address + 1)` and overflows on it, which
        // would panic a debug build rather than report an unattributable address.
        let mapper = SourceMapper::new(DWARF_PROBE).expect("the fixture carries DWARF");

        for pc in [
            0usize,
            1,
            16,
            17,
            157,
            166,
            4096,
            1 << 20,
            usize::MAX - 1,
            usize::MAX,
        ] {
            assert_eq!(mapper.resolve(pc), None, "pc {pc} should not resolve");
        }
    }
    #[test]
    fn an_unmapped_mapper_resolves_nothing_at_any_address() {
        let mapper = SourceMapper::unmapped();

        assert!(!mapper.has_debug_info());
        for pc in [0usize, 1, 64, 4096, usize::MAX] {
            assert!(
                mapper.resolve(pc).is_none(),
                "pc {pc} resolved to a frame from a mapper with no symbols"
            );
        }
    }

    #[test]
    fn section_walk_survives_a_module_that_ends_on_a_boundary() {
        // The loop must stop cleanly at the last section's end byte rather than reading one past.
        let exact = module(&[(".debug_info", b"\x00")]);

        assert_eq!(
            WasmSections::parse(&exact).unwrap().get(".debug_info"),
            Some(&[0u8][..])
        );
    }
}
