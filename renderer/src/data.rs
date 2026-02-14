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
    pub generated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct Branch {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub branch: String,
    pub message: String,
    pub parents: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub conventional_type: Option<String>,
    pub ticket_id: Option<String>,
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
    pub to_branch: String,
    pub timestamp: DateTime<Utc>,
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
    pub by_branch: std::collections::HashMap<String, u32>,
    #[serde(default)]
    pub top_authors: Vec<AuthorEntry>,
    #[serde(default)]
    pub release_cycles: ReleaseCycleStats,
}

#[derive(Debug, Deserialize)]
pub struct CollectedData {
    pub metadata: Metadata,
    pub branches: Vec<Branch>,
    pub commits: Vec<Commit>,
    pub merges: Vec<Merge>,
    #[serde(default)]
    pub deployments: Vec<serde_json::Value>,
    #[serde(default)]
    pub ci_runs: Vec<serde_json::Value>,
    pub statistics: Option<Statistics>,
}

pub fn load_data(path: &Path) -> Result<CollectedData, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let data: CollectedData = serde_json::from_str(&contents)?;
    Ok(data)
}
