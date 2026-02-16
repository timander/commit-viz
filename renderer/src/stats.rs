use crate::data::CollectedData;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

/// Rolling "code inventory" metrics for the stats overlay, one per visible_count.
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    pub unmerged_commits: u32,
    pub active_branches: u32,
    pub stale_branches: u32,
    pub unmerged_lines: u64,
    pub unmerged_files: u32,
    pub integration_debt: u64,
    pub days_since_release: f64,
    pub awaiting_release: u32,
    pub oldest_unmerged_days: f64,
    pub merge_throughput: u32,
}

/// Per-branch tracking state used during incremental computation.
struct BranchState {
    commits: u32,
    lines: u64,
    files: u32,
    first_commit_time: DateTime<Utc>,
    last_commit_time: DateTime<Utc>,
    merged: bool,
}

/// Pre-compute one `FrameStats` for every `visible_count` from 1..=commits.len().
/// This is called once before the render loop and indexed per-frame.
pub fn precompute_frame_stats(data: &CollectedData, default_branch: &str) -> Vec<FrameStats> {
    let num_commits = data.commits.len();
    if num_commits == 0 {
        return Vec::new();
    }

    // Build a set of merge shas for quick lookup, and track which branches get merged at which commit index.
    let merge_shas: HashSet<&str> = data.merges.iter().map(|m| m.sha.as_str()).collect();
    // Map from merge sha -> from_branch (the branch being merged in)
    let merge_from: HashMap<&str, &str> = data
        .merges
        .iter()
        .map(|m| (m.sha.as_str(), m.from_branch.as_str()))
        .collect();

    let mut branch_states: HashMap<String, BranchState> = HashMap::new();
    let mut last_release_time: Option<DateTime<Utc>> = None;
    let mut main_commits_after_last_tag: u32 = 0;
    // Track merges with timestamps for rolling 30-day throughput
    let mut merge_times: Vec<DateTime<Utc>> = Vec::new();

    let mut results = Vec::with_capacity(num_commits);

    for i in 0..num_commits {
        let commit = &data.commits[i];
        let branch = &commit.branch;
        let is_default = branch == default_branch;
        let now = commit.timestamp;

        // Update branch state
        let state = branch_states
            .entry(branch.clone())
            .or_insert_with(|| BranchState {
                commits: 0,
                lines: 0,
                files: 0,
                first_commit_time: now,
                last_commit_time: now,
                merged: is_default, // default branch is always "merged"
            });

        state.commits += 1;
        state.lines += (commit.insertions + commit.deletions) as u64;
        state.files += commit.files_changed;
        state.last_commit_time = now;

        // Check if this commit is a merge that resolves a branch
        if merge_shas.contains(commit.sha.as_str()) {
            merge_times.push(now);
            if let Some(from_branch) = merge_from.get(commit.sha.as_str()) {
                if let Some(bs) = branch_states.get_mut(*from_branch) {
                    bs.merged = true;
                }
            }
        }

        // Track tags/releases
        if is_default && !commit.tags.is_empty() {
            last_release_time = Some(now);
            main_commits_after_last_tag = 0;
        } else if is_default {
            main_commits_after_last_tag += 1;
        }

        // Now compute FrameStats from current branch_states
        let mut unmerged_commits: u32 = 0;
        let mut active_branches: u32 = 0;
        let mut stale_branches: u32 = 0;
        let mut unmerged_lines: u64 = 0;
        let mut unmerged_files: u32 = 0;
        let mut integration_debt: u64 = 0;
        let mut oldest_unmerged_days: f64 = 0.0;

        for (bname, bs) in &branch_states {
            if bname == default_branch {
                continue;
            }
            if bs.merged {
                continue;
            }

            // This branch is unmerged
            unmerged_commits += bs.commits;
            unmerged_lines += bs.lines;
            unmerged_files += bs.files;

            let age_days = (now - bs.first_commit_time).num_seconds() as f64 / 86400.0;
            let since_last = (now - bs.last_commit_time).num_seconds() as f64 / 86400.0;

            // Integration debt = lines * age_days
            integration_debt += (bs.lines as f64 * age_days) as u64;

            if since_last <= 30.0 {
                active_branches += 1;
            } else {
                stale_branches += 1;
            }

            if age_days > oldest_unmerged_days {
                oldest_unmerged_days = age_days;
            }
        }

        let days_since_release = match last_release_time {
            Some(t) => (now - t).num_seconds() as f64 / 86400.0,
            None => {
                // No release yet — days since first commit
                (now - data.commits[0].timestamp).num_seconds() as f64 / 86400.0
            }
        };

        // Merge throughput: merges in the last 30 days of the visible window
        let cutoff = now - chrono::Duration::days(30);
        let merge_throughput = merge_times.iter().filter(|&&t| t >= cutoff).count() as u32;

        results.push(FrameStats {
            unmerged_commits,
            active_branches,
            stale_branches,
            unmerged_lines,
            unmerged_files,
            integration_debt,
            days_since_release,
            awaiting_release: main_commits_after_last_tag,
            oldest_unmerged_days,
            merge_throughput,
        });
    }

    results
}
