# OCEL Gap Report — anti-llm-lsp

## Scope
Verification of checkpoint `OCEL-COMPAT-001`.

## Object Types Coverage
All 20 object types defined in the specification are fully mapped:
- `Repository`, `Crate`, `File`, `FileRange`, `Diagnostic`, `DiagnosticCode`, `ForbiddenImplication`, `Checkpoint`, `Receipt`, `Digest`, `JsonRpcTranscript`, `LspMethod`, `LspCapabilityPath`, `Lsp318FeatureRow`, `NegativeControlFixture`, `TestCase`, `Failset`, `AgentReport`, `RouteStage`, `OcelEventLog`

## Event Types Coverage
All 19 event types defined in the specification are fully mapped:
- `RepositoryScanned`, `FileObserved`, `RawTextObservationDetected`, `TreeSitterObservationDetected`, `CargoGraphObservationDetected`, `MarkdownClaimDetected`, `JsonRpcTranscriptParsed`, `ReceiptFileParsed`, `RouteEvidenceChecked`, `DiagnosticEmitted`, `ForbiddenImplicationDetected`, `NegativeControlExecuted`, `Lsp318FeatureExercised`, `Lsp318FeatureRefusedByLaw`, `VirtualDocumentServed`, `FailsetUpdated`, `ClaimStatusReported`, `ReceiptValidated`, `AuditReportScanned`

## Core Relations Mapping
The 5 key relations are mapped:
- `DiagnosticEmitted` -> `FileRange`, `DiagnosticCode`, `ForbiddenImplication`, `Checkpoint`
- `ReceiptValidated` -> `Receipt`, `Digest`, `Checkpoint`
- `Lsp318FeatureExercised` -> `Lsp318FeatureRow`, `JsonRpcTranscript`, `Receipt`
- `NegativeControlExecuted` -> `NegativeControlFixture`, `DiagnosticCode`, `Receipt`
- `FailsetUpdated` -> `Diagnostic`, `Checkpoint`, `AgentReport`

## Gaps
There are no gaps between the actual event log implementation and the specification rules.
All types are correctly emitted and matched.
