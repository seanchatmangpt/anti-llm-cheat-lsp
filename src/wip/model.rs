use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const WIP_SNAPSHOT_SCHEMA: &str = "chatman.wip.github-snapshot/v1";
pub const WIP_REPORT_SCHEMA: &str = "chatman.wip.report/v1";
pub const DEFAULT_STALE_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Standing {
    Unknown,
    PartialAlive,
    Alive,
    Blocked,
    BuildBroken,
    Unsupported,
    Refused,
}

impl Default for Standing {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WipKind {
    Code,
    Test,
    Integration,
    Dependency,
    Ci,
    Review,
    Merge,
    Release,
    Documentation,
    Evidence,
    Receipt,
    Replay,
    GeneratedProjection,
    OrphanBranch,
    StalePr,
    AbandonedExperiment,
    DuplicateImplementation,
    UnsatisfiedRequirement,
    CrossRepoBlocker,
    SourceMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClosureActionKind {
    RepairCi,
    FinishImplementation,
    ResolveBlocker,
    MaterializeDependency,
    AddEvidence,
    AddReceipt,
    ReplaceReplayPointer,
    RequestReview,
    Merge,
    Release,
    OpenPullRequest,
    CompareAndSupersede,
    CloseSatisfiedIssue,
    ReclassifyUnsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubSnapshot {
    pub schema_version: String,
    pub observed_at: DateTime<Utc>,
    #[serde(default = "default_window_days")]
    pub window_days: u32,
    /// Repository whose checked-out source tree is scanned alongside this snapshot.
    ///
    /// Required for unambiguous source attribution when `repositories` contains more than one repo.
    pub workspace_repository: Option<String>,
    #[serde(default)]
    pub repositories: Vec<RepositorySnapshot>,
}

const fn default_window_days() -> u32 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepositorySnapshot {
    pub full_name: String,
    #[serde(default = "default_branch_name")]
    pub default_branch: String,
    #[serde(default)]
    pub branches: Vec<BranchSnapshot>,
    #[serde(default)]
    pub pull_requests: Vec<PullRequestSnapshot>,
    #[serde(default)]
    pub issues: Vec<IssueSnapshot>,
    #[serde(default)]
    pub workflow_runs: Vec<WorkflowRunSnapshot>,
    #[serde(default)]
    pub releases: Vec<ReleaseSnapshot>,
    #[serde(default)]
    pub commits: Vec<CommitSnapshot>,
    #[serde(default)]
    pub dependency_edges: Vec<DependencyEdge>,
}

fn default_branch_name() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BranchSnapshot {
    pub name: String,
    #[serde(default)]
    pub head_sha: String,
    #[serde(default)]
    pub ahead_by: u64,
    #[serde(default)]
    pub behind_by: u64,
    #[serde(default)]
    pub merged_into_default: bool,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub associated_pr: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PullRequestSnapshot {
    pub number: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub head_branch: String,
    #[serde(default)]
    pub base_branch: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub merged_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ci_status: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub changed_files: u64,
    #[serde(default)]
    pub linked_issues: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IssueSnapshot {
    pub number: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub state: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub linked_pr: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowRunSnapshot {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub conclusion: String,
    #[serde(default)]
    pub head_branch: String,
    #[serde(default)]
    pub head_sha: String,
    pub pull_request: Option<u64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseSnapshot {
    #[serde(default)]
    pub tag_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitSnapshot {
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub message: String,
    pub committed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub relation: String,
    #[serde(default)]
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFinding {
    pub id: String,
    pub path: String,
    pub line: usize,
    pub marker: String,
    pub kind: WipKind,
    pub standing: Standing,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipObject {
    pub id: String,
    pub repository: String,
    pub kind: WipKind,
    pub standing: Standing,
    pub title: String,
    pub origin: String,
    pub created_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub age_days: f64,
    pub blockers: Vec<String>,
    pub dependents: Vec<String>,
    pub evidence: Vec<String>,
    pub closure_conditions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosureIntent {
    pub id: String,
    pub wip_id: String,
    pub repository: String,
    pub action: ClosureActionKind,
    pub reason: String,
    pub expected_wip_reduction: f64,
    pub blocked_dependents: usize,
    pub age_days: f64,
    pub estimated_cost: f64,
    pub priority_score: f64,
    pub authority: String,
    pub receipt_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LittleLawMetrics {
    /// L: admitted WIP objects observed in the snapshot.
    pub wip_l: usize,
    /// lambda: observed closures per day within the admitted observation window.
    pub throughput_lambda_per_day: Option<f64>,
    /// W: mean observed cycle time in days for completed PRs/issues in-window.
    pub mean_cycle_time_days: Option<f64>,
    /// L/lambda, only when lambda is observed and non-zero.
    pub projected_wip_time_days: Option<f64>,
    pub observation_window_days: u32,
    pub completed_objects_in_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipReport {
    pub schema_version: String,
    pub observed_at: DateTime<Utc>,
    pub status: Standing,
    pub metrics: LittleLawMetrics,
    pub wip: Vec<WipObject>,
    pub closure_frontier: Vec<ClosureIntent>,
    pub source_findings: Vec<SourceFinding>,
    pub notes: Vec<String>,
}
