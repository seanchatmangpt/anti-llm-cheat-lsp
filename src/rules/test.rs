use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

pub fn evaluate(
    obs: &[Observation],
    config: &crate::config::AntiLlmConfig,
) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // assert_contains_string: .contains("literal") — Display/string cheat
        // assert_contains: unknown receiver type — flag conservatively
        // assert_contains_structural: .contains(&EnumVariant) — acceptable, no diagnostic
        if (o.construct == "assert_contains_string" || o.construct == "assert_contains")
            && !config.test_is_structural_path(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-TEST-001".to_string(),
                category: "test".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "String assertion in test file. Tests must verify structural \
                    properties instead of substring search on Display outputs."
                    .to_string(),
                forbidden_implication: "TestStdout => Receipt".to_string(),
                blocking: true,
                required_correction:
                    "Replace assert!(x.to_string().contains(\"Name\")) with \
                    assert_eq!(x, Enum::Variant). For Vec membership use \
                    assert!(vec.contains(&Enum::Variant)) — the scanner detects that as structural."
                        .to_string(),
                required_next_proof: "Run cargo test to verify structural assertions hold."
                    .to_string(),
            });
        }

        // assert_contains_structural: Vec::contains(&EnumVariant) — no diagnostic.
        // The scanner emits this construct so call sites can see it was classified,
        // but no CHEAT diagnostic fires.

        // TEST-002: assert!(expr.is_ok()) with no value extraction.
        if o.construct == "assert_is_ok_only" && !config.test_is_structural_path(&o.file_path) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-TEST-002".to_string(),
                category: "test".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "assert!(expr.is_ok()) proves absence of panic, not correctness. \
                    Couple with a structural check on the Ok payload or post-call state \
                    (e.g. assert_eq!(engine.current_zone(), Zone::X))."
                    .to_string(),
                forbidden_implication: "NoErrorRaised => BehaviorCorrect".to_string(),
                blocking: false,
                required_correction:
                    "Extract the Ok value and assert a structural property on it, \
                    OR assert state change on the system under test after the call."
                        .to_string(),
                required_next_proof: "Test still passes after removing the is_ok() and replacing \
                    with a check that would fail if the SUT returned a wrong-but-Ok value."
                    .to_string(),
            });
        }

        // TEST-004: error-swallowing unwrap_or_default() / unwrap_or() in test code.
        if o.construct == "test_unwrap_or_swallow" && !config.test_is_structural_path(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-TEST-004".to_string(),
                category: "test".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "unwrap_or_default() / unwrap_or() in test helper silently converts \
                    SUT errors into empty/zero values — downstream assertions pass \
                    vacuously when the SUT actually failed."
                    .to_string(),
                forbidden_implication: "HelperReturnsDefault => SUTSucceeded".to_string(),
                blocking: true,
                required_correction: "Use .expect(\"descriptive message\") or propagate the \
                    error with ?. The test must fail loudly when the SUT returns Err."
                    .to_string(),
                required_next_proof: "Replace unwrap_or_default() with expect(); cargo test \
                    still passes for the passing case and fails for injected error case."
                    .to_string(),
            });
        }

        // TEST-005: count-match assertion assert_eq!(X.len(), N).
        if o.construct.starts_with("assert_len_literal_")
            && !config.test_is_structural_path(&o.file_path)
        {
            let n = o.construct.trim_start_matches("assert_len_literal_");
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-TEST-005".to_string(),
                category: "test".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!(
                    "assert_eq!(...len(), {n}) verifies quantity not quality — an LLM that \
                    generates exactly {n} items trivially passes this. Combine with at least \
                    one structural check on a specific element."
                ),
                forbidden_implication: "CountMatchesSpec => ElementsAreCorrect".to_string(),
                blocking: false,
                required_correction: format!(
                    "Add an assertion on a specific element: e.g. assert_eq!(items[0].kind, \
                    ExpectedKind::X) alongside the assert_eq!(items.len(), {n})."
                ),
                required_next_proof: "Injecting a wrong-typed element at index 0 causes the \
                    new structural assertion to fail while the len() assertion still passes."
                    .to_string(),
            });
        }

        // TEST-006: snapshot-as-spec — assert_snapshot! / assert_yaml_snapshot!
        // without a structural assert_eq!/assert_ne! on the same binding creates
        // circular specs where LLM output is promoted to expected value.
        if o.construct == "assert_snapshot_no_eq" && !config.test_is_structural_path(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-TEST-006".to_string(),
                category: "test".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "assert_snapshot!/assert_yaml_snapshot! without a structural \
                    assert_eq!/assert_ne! creates a circular spec: the LLM's own output \
                    becomes the expected value, making the test unfalsifiable."
                    .to_string(),
                forbidden_implication: "SnapshotAccepted => OutputCorrect".to_string(),
                blocking: true,
                required_correction:
                    "Add assert_eq!(binding, ExpectedType::Variant) alongside the snapshot \
                    call, or replace the snapshot entirely with a structural assertion that \
                    would fail if the SUT returned a wrong-but-serializable value."
                        .to_string(),
                required_next_proof:
                    "Injecting a wrong value into the SUT causes the structural assertion \
                    to fail even though the snapshot would still match."
                        .to_string(),
            });
        }

        if o.construct == "negative_control_reference" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-TEST-003".to_string(),
                category: "test".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Standard test references or verifies negative controls directly. \
                    This is prohibited except in authorized dogfooding tests."
                    .to_string(),
                forbidden_implication: "Positive case passes => law holds".to_string(),
                blocking: true,
                required_correction:
                    "Remove negative control directory or fixture references from standard tests."
                        .to_string(),
                required_next_proof:
                    "Run test suite to verify tests use mocked or isolated test fixtures."
                        .to_string(),
            });
        }
    }

    diags
}
