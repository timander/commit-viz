use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub repo: String,
    pub date_range: DateRange,
}

#[derive(Debug, Deserialize)]
pub struct Branch {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub timestamp: DateTime<Utc>,
    pub branch: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub insertions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(default)]
    pub files_changed: u32,
    #[serde(default = "default_category")]
    pub category: String,
}

fn default_category() -> String {
    "other".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Merge {
    pub sha: String,
    pub from_branch: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReleaseCycleStats {
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub mean_days: f64,
    #[serde(default)]
    pub min_days: f64,
    #[serde(default)]
    pub max_days: f64,
    #[serde(default)]
    pub stdev_days: f64,
}

#[derive(Debug, Deserialize, Default)]
pub struct AuthorEntry {
    pub author: String,
    pub commits: u32,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct CommitToReleaseDayEntry {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub avg_days_to_release: f64,
    #[serde(default)]
    pub unreleased_count: u32,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct BranchLifespan {
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub first_commit: String,
    #[serde(default)]
    pub last_commit: String,
    #[serde(default)]
    pub lifespan_days: f64,
    #[serde(default)]
    pub merged: bool,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct DailyVelocity {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub dominant_category: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct DroughtPeriod {
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub duration_days: u32,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct CommitMergeLatencyEntry {
    #[serde(default)]
    pub commit_date: String,
    pub days_to_merge: Option<f64>,
    #[serde(default)]
    pub lines_changed: u32,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ReleaseInterval {
    #[serde(default)]
    pub days_since_previous: f64,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct HistogramBin {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub count: u32,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct WorkDispositionSegment {
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub merge_speed: String,
    #[serde(default)]
    pub lines_changed: u32,
    #[serde(default)]
    pub commit_count: u32,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct WorkDisposition {
    #[serde(default)]
    pub fast_merged_lines: u32,
    #[serde(default)]
    pub slow_merged_lines: u32,
    #[serde(default)]
    pub unmerged_lines: u32,
    #[serde(default)]
    pub fast_merged_commits: u32,
    #[serde(default)]
    pub slow_merged_commits: u32,
    #[serde(default)]
    pub unmerged_commits: u32,
    #[serde(default)]
    pub segments: Vec<WorkDispositionSegment>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct RollingAvgEntry {
    #[serde(default)]
    pub avg: f64,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct ChangeFlowMetrics {
    #[serde(default)]
    pub commit_to_release_days: Vec<CommitToReleaseDayEntry>,
    #[serde(default)]
    pub release_median_latency: f64,
    #[serde(default)]
    pub release_p90_latency: f64,
    #[serde(default)]
    pub release_pct_within_7d: f64,
    #[serde(default)]
    pub branch_lifespans: Vec<BranchLifespan>,
    #[serde(default)]
    pub branch_median_lifespan: f64,
    #[serde(default)]
    pub branch_unmerged_count: u32,
    #[serde(default)]
    pub branch_longest_days: f64,
    #[serde(default)]
    pub daily_velocity: Vec<DailyVelocity>,
    #[serde(default)]
    pub rolling_7day_avg: Vec<RollingAvgEntry>,
    #[serde(default)]
    pub drought_periods: Vec<DroughtPeriod>,
    #[serde(default)]
    pub drought_count: u32,
    #[serde(default)]
    pub longest_drought_days: u32,
    #[serde(default)]
    pub total_drought_days: u32,
    #[serde(default)]
    pub commit_merge_latency: Vec<CommitMergeLatencyEntry>,
    #[serde(default)]
    pub merge_median_latency: f64,
    #[serde(default)]
    pub merge_pct_within_7d: f64,
    #[serde(default)]
    pub merge_pct_within_30d: f64,
    #[serde(default)]
    pub release_intervals: Vec<ReleaseInterval>,
    #[serde(default)]
    pub release_interval_distribution: Vec<HistogramBin>,
    #[serde(default)]
    pub release_interval_mean: f64,
    #[serde(default)]
    pub release_interval_median: f64,
    #[serde(default)]
    pub release_interval_cv: f64,
    #[serde(default)]
    pub release_interval_longest_gap: f64,
    #[serde(default)]
    pub work_disposition: WorkDisposition,
}

#[derive(Debug, Deserialize, Default)]
pub struct Statistics {
    #[serde(default)]
    pub total_commits: u32,
    #[serde(default)]
    pub date_span_days: u32,
    #[serde(default)]
    pub commits_per_week: f64,
    #[serde(default)]
    pub unique_authors: u32,
    #[serde(default)]
    pub by_category: std::collections::HashMap<String, u32>,
    #[serde(default)]
    pub top_authors: Vec<AuthorEntry>,
    #[serde(default)]
    pub release_cycles: ReleaseCycleStats,
    pub change_flow: Option<ChangeFlowMetrics>,
}

#[derive(Debug, Deserialize)]
pub struct CollectedData {
    pub metadata: Metadata,
    pub branches: Vec<Branch>,
    pub commits: Vec<Commit>,
    pub merges: Vec<Merge>,
    pub statistics: Option<Statistics>,
}

pub fn load_data(path: &Path) -> Result<CollectedData, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let data: CollectedData = serde_json::from_str(&contents)?;
    Ok(data)
}
