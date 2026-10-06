//! End-to-end check of the differential artifact (#44).
//!
//! The unit tests in `src/formatter.rs` cover the differ's rules against inline strings.
//! This file covers what a consumer actually does: read two committed profiling artifacts
//! out of `tests/traces/`, diff them, and confirm each stack lands on the expected side of
//! the red/blue scale.

use soroban_cost_profiler::formatter::{DeltaScale, OutputFormatter};

fn trace(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/traces")
        .join(name)
        .with_extension("folded");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} should ship with the repo: {error}", path.display()))
}

/// Each stack paired with the color scale its baseline/current counts classify to.
fn classified(diff: &str) -> Vec<(String, DeltaScale)> {
    diff.lines()
        .map(|line| {
            let mut fields = line.split(' ');
            let (path, baseline, current) = (
                fields.next().unwrap().to_string(),
                fields.next().unwrap().parse().unwrap(),
                fields.next().unwrap().parse().unwrap(),
            );
            assert!(
                fields.next().is_none(),
                "differential lines must be `<stack> <baseline> <current>`, got {line:?}"
            );
            (path, OutputFormatter::delta_scale(baseline, current))
        })
        .collect()
}

#[test]
fn the_sample_artifacts_are_valid_folded_stacks() {
    let baseline = OutputFormatter::parse_folded(&trace("baseline")).unwrap();
    let current = OutputFormatter::parse_folded(&trace("current")).unwrap();

    assert_eq!(baseline.get("main"), Some(&10));
    assert_eq!(baseline.get("main;compute_heavy_loop"), Some(&40_000));
    assert_eq!(current.get("main;compute_heavy_loop"), Some(&60_000));
}

#[test]
fn the_diff_carries_both_counts_per_stack_in_stable_order() {
    let diff = OutputFormatter::to_differential_folded(&trace("baseline"), &trace("current"))
        .expect("both sample traces are well-formed");

    assert_eq!(
        diff,
        "main 10 10\n\
         main;added_helper 0 900\n\
         main;compute_heavy_loop 40000 60000\n\
         main;memory_heavy_loop 8000 4000\n\
         main;retired_helper 500 0\n"
    );
}

#[test]
fn each_sample_stack_maps_to_the_expected_color() {
    let diff =
        OutputFormatter::to_differential_folded(&trace("baseline"), &trace("current")).unwrap();

    let scales = classified(&diff);
    let expected = [
        // Unchanged cost stays neutral.
        ("main", DeltaScale::Neutral),
        // A frame that did not exist in the baseline is pure new cost.
        ("main;added_helper", DeltaScale::Regression),
        // The regression the feature exists to surface.
        ("main;compute_heavy_loop", DeltaScale::Regression),
        // And its counterpart: the same frame got cheaper.
        ("main;memory_heavy_loop", DeltaScale::Improvement),
        // A frame removed since the baseline shows as fully improved, not as absent.
        ("main;retired_helper", DeltaScale::Improvement),
    ];

    for (path, scale) in expected {
        let found = scales
            .iter()
            .find(|(found_path, _)| found_path == path)
            .unwrap_or_else(|| panic!("{path} should appear in the diff, got: {diff}"));
        assert_eq!(
            found.1, scale,
            "{path} should be {scale:?}, got {:?} in:\n{diff}",
            found.1
        );
    }
}

#[test]
fn diffing_a_trace_against_itself_reports_no_change() {
    // The baseline-vs-baseline case is what a CI job runs when nothing regressed, so it has
    // to produce a perfectly neutral artifact rather than noise.
    let current = trace("current");
    let diff = OutputFormatter::to_differential_folded(&current, &current).unwrap();

    assert!(
        classified(&diff)
            .into_iter()
            .all(|(_, scale)| scale == DeltaScale::Neutral),
        "a trace diffed against itself should be all-neutral, got:\n{diff}"
    );
}
