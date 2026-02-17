use crate::data::{CollectedData, Commit};
use chrono::Datelike;
use std::collections::HashMap;

pub struct PositionedCommit<'a> {
    pub commit: &'a Commit,
    pub x: f32,
    pub y: f32,
    pub slot: usize,
    pub rect_w: f32,
    pub rect_h: f32,
    pub is_default_branch: bool,
    pub branch_has_conflicts: bool,
    pub branch_is_stale: bool,
}

pub struct PositionedMerge {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub slot: usize,
    pub has_conflicts: bool,
    pub is_stale: bool,
}

pub struct DateTick {
    pub x: f32,
    pub label: String,
}

pub struct BranchLabel {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub slot: usize,
    pub has_conflicts: bool,
    pub is_stale: bool,
}

pub struct PositionedTag {
    pub x: f32,
    pub main_y: f32,
    pub label_y: f32,
    pub tag_name: String,
}

pub struct BranchVisualInfo {
    pub name: String,
    pub slot: usize,
    pub has_conflicts: bool,
    pub is_stale: bool,
    pub base_y: f32,
    pub parent_branch: Option<String>,
}

pub struct NetworkLayout {
    pub width: u32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub main_y: f32,
    pub min_branch_spacing: f32,
    pub max_divergence_offset: f32,
    pub default_branch: String,
}

const MIN_RECT_W: f32 = 4.0;
const MAX_RECT_W: f32 = 20.0;
const MIN_RECT_H: f32 = 4.0;
const MAX_RECT_H: f32 = 24.0;
const MIN_BRANCH_SPACING: f32 = 35.0;
const MAX_DIVERGENCE_OFFSET: f32 = 250.0;

/// Tracks cumulative divergence stats per branch during positioning.
struct BranchDivergenceState {
    slot: usize,
    cum_commits: u32,
    cum_lines: u64,
    cum_files: u32,
    has_conflicts: bool,
    is_stale: bool,
    last_commit_timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl NetworkLayout {
    pub fn from_data(data: &CollectedData, width: u32, height: u32) -> Self {
        let margin_top = 70.0;
        let margin_bottom = 120.0;
        let margin_left = 80.0;
        let margin_right = 40.0;

        let default_branch = data
            .branches
            .iter()
            .find(|b| b.is_default)
            .map_or_else(|| "main".to_string(), |b| b.name.clone());

        let usable_height = height as f32 - margin_top - margin_bottom;
        // Main at ~15% down from top of usable area, leaving room above for tags
        let main_y = margin_top + usable_height * 0.15;

        NetworkLayout {
            width,
            margin_left,
            margin_right,
            main_y,
            min_branch_spacing: MIN_BRANCH_SPACING,
            max_divergence_offset: MAX_DIVERGENCE_OFFSET,
            default_branch,
        }
    }

    fn commit_to_x(&self, index: usize, total: usize) -> f32 {
        let usable = self.width as f32 - self.margin_left - self.margin_right;
        if total <= 1 {
            return self.margin_left + usable / 2.0;
        }
        self.margin_left + (index as f32 / (total - 1) as f32) * usable
    }

    fn commit_rect(commit: &Commit) -> (f32, f32) {
        let files = commit.files_changed.max(1) as f32;
        let lines = (commit.insertions + commit.deletions).max(1) as f32;

        let w = (files.ln() / 10.0_f32.ln()) * (MAX_RECT_W - MIN_RECT_W) + MIN_RECT_W;
        let h = (lines.ln() / 10.0_f32.ln()) * (MAX_RECT_H - MIN_RECT_H) + MIN_RECT_H;

        (
            w.clamp(MIN_RECT_W, MAX_RECT_W),
            h.clamp(MIN_RECT_H, MAX_RECT_H),
        )
    }

    /// Compute dynamic Y for a branch commit based on cumulative divergence
    /// relative to the parent branch's `base_y`.
    fn divergence_y(&self, state: &BranchDivergenceState, parent_base_y: f32) -> f32 {
        let divergence = (1.0 + f64::from(state.cum_commits)).log2() as f32 * 15.0
            + (1.0 + state.cum_lines as f64).log2() as f32 * 8.0
            + (1.0 + f64::from(state.cum_files)).log2() as f32 * 5.0;
        let clamped = divergence.min(self.max_divergence_offset);
        // Branches go BELOW their parent
        parent_base_y + self.min_branch_spacing + clamped
    }

    /// Walk commits chronologically, assign slots on first appearance,
    /// compute dynamic Y per commit based on cumulative branch divergence.
    pub fn position_commits_dynamic<'a>(
        &self,
        data: &'a CollectedData,
    ) -> (Vec<PositionedCommit<'a>>, Vec<BranchVisualInfo>) {
        let total = data.commits.len();
        let mut branch_states: HashMap<String, BranchDivergenceState> = HashMap::new();
        let mut result = Vec::with_capacity(total);

        // Build merge set to detect which branches get merged
        let merge_from_branches: std::collections::HashSet<&str> =
            data.merges.iter().map(|m| m.from_branch.as_str()).collect();

        // Detect conflict branches: any branch that has a commit with category "conflict"
        let mut conflict_branches: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for c in &data.commits {
            if c.category == "conflict" {
                conflict_branches.insert(c.branch.clone());
            }
        }

        // First pass: detect stale branches (unmerged, last commit > 30 days before repo end)
        let repo_end = data.commits.last().map(|c| c.timestamp);
        let mut branch_last_commit: HashMap<String, chrono::DateTime<chrono::Utc>> = HashMap::new();
        for c in &data.commits {
            branch_last_commit
                .entry(c.branch.clone())
                .and_modify(|t| {
                    if c.timestamp > *t {
                        *t = c.timestamp;
                    }
                })
                .or_insert(c.timestamp);
        }

        let stale_branches: std::collections::HashSet<String> = if let Some(end) = repo_end {
            branch_last_commit
                .iter()
                .filter(|(name, last)| {
                    *name != &self.default_branch
                        && !merge_from_branches.contains(name.as_str())
                        && (end - **last).num_days() > 30
                })
                .map(|(name, _)| name.clone())
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        // Build parent_branch map from data
        let parent_branch_map: HashMap<&str, &str> = data
            .branches
            .iter()
            .filter_map(|b| {
                b.parent_branch
                    .as_ref()
                    .map(|pb| (b.name.as_str(), pb.as_str()))
            })
            .collect();

        // DFS tree traversal for hierarchical slot assignment
        // Build children map: parent -> Vec<child>
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for b in &data.branches {
            if b.name == self.default_branch {
                continue;
            }
            let parent = parent_branch_map
                .get(b.name.as_str())
                .copied()
                .unwrap_or(self.default_branch.as_str());
            children.entry(parent).or_default().push(b.name.as_str());
        }
        // Sort children alphabetically at each level
        for v in children.values_mut() {
            v.sort_unstable();
        }

        // DFS from default branch to assign slots
        let mut dfs_order: Vec<&str> = Vec::new();
        let mut dfs_stack: Vec<&str> = Vec::new();
        // Push children of default branch in reverse order (so first alphabetically pops first)
        if let Some(kids) = children.get(self.default_branch.as_str()) {
            for &kid in kids.iter().rev() {
                dfs_stack.push(kid);
            }
        }
        // Also handle branches whose parent isn't in our branch list (orphans → treat as children of default)
        let branch_name_set: std::collections::HashSet<&str> =
            data.branches.iter().map(|b| b.name.as_str()).collect();
        let mut orphans: Vec<&str> = data
            .branches
            .iter()
            .filter(|b| {
                b.name != self.default_branch
                    && parent_branch_map
                        .get(b.name.as_str())
                        .is_none_or(|p| !branch_name_set.contains(p))
                    && !children
                        .get(self.default_branch.as_str())
                        .is_some_and(|kids| kids.contains(&b.name.as_str()))
            })
            .map(|b| b.name.as_str())
            .collect();
        orphans.sort_unstable();
        for &orphan in orphans.iter().rev() {
            dfs_stack.push(orphan);
        }

        while let Some(branch) = dfs_stack.pop() {
            dfs_order.push(branch);
            if let Some(kids) = children.get(branch) {
                for &kid in kids.iter().rev() {
                    dfs_stack.push(kid);
                }
            }
        }

        let branch_slot_map: HashMap<&str, usize> = dfs_order
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, i))
            .collect();

        // Compute hierarchical base_y for each branch (DFS order guarantees parent computed first)
        let mut branch_base_y: HashMap<&str, f32> = HashMap::new();
        branch_base_y.insert(self.default_branch.as_str(), self.main_y);
        for &branch_name in &dfs_order {
            let parent = parent_branch_map
                .get(branch_name)
                .copied()
                .unwrap_or(self.default_branch.as_str());
            let parent_y = branch_base_y.get(parent).copied().unwrap_or(self.main_y);
            let base_y = parent_y + self.min_branch_spacing;
            branch_base_y.insert(branch_name, base_y);
        }

        // Initialize branch_states for all non-default branches
        for &name in &dfs_order {
            let slot = branch_slot_map.get(name).copied().unwrap_or(0);
            branch_states.insert(
                name.to_string(),
                BranchDivergenceState {
                    slot,
                    cum_commits: 0,
                    cum_lines: 0,
                    cum_files: 0,
                    has_conflicts: conflict_branches.contains(name),
                    is_stale: stale_branches.contains(name),
                    last_commit_timestamp: None,
                },
            );
        }

        for (i, commit) in data.commits.iter().enumerate() {
            let x = self.commit_to_x(i, total);
            let is_default = commit.branch == self.default_branch;

            let (y, slot, has_conflicts, is_stale) = if is_default {
                (self.main_y, 0, false, false)
            } else {
                let slot = branch_slot_map
                    .get(commit.branch.as_str())
                    .copied()
                    .unwrap_or(0);
                let state = branch_states
                    .entry(commit.branch.clone())
                    .or_insert_with(|| BranchDivergenceState {
                        slot,
                        cum_commits: 0,
                        cum_lines: 0,
                        cum_files: 0,
                        has_conflicts: conflict_branches.contains(&commit.branch),
                        is_stale: stale_branches.contains(&commit.branch),

                        last_commit_timestamp: None,
                    });

                state.cum_commits += 1;
                state.cum_lines += u64::from(commit.insertions + commit.deletions);
                state.cum_files += commit.files_changed;
                state.last_commit_timestamp = Some(commit.timestamp);

                let parent_y = branch_base_y
                    .get(commit.branch.as_str())
                    .and_then(|_| {
                        parent_branch_map
                            .get(commit.branch.as_str())
                            .and_then(|p| branch_base_y.get(p))
                    })
                    .copied()
                    .unwrap_or(self.main_y);
                let y = self.divergence_y(state, parent_y);
                (y, state.slot, state.has_conflicts, state.is_stale)
            };

            let (rect_w, rect_h) = Self::commit_rect(commit);

            result.push(PositionedCommit {
                commit,
                x,
                y,
                slot,
                rect_w,
                rect_h,
                is_default_branch: is_default,
                branch_has_conflicts: has_conflicts,
                branch_is_stale: is_stale,
            });
        }

        // Build branch visual info with hierarchical base_y for phantom rendering
        let branch_infos: Vec<BranchVisualInfo> = branch_states
            .into_iter()
            .map(|(name, state)| {
                let base_y = branch_base_y
                    .get(name.as_str())
                    .copied()
                    .unwrap_or(self.main_y + self.min_branch_spacing);
                let parent = parent_branch_map
                    .get(name.as_str())
                    .map(std::string::ToString::to_string);
                BranchVisualInfo {
                    name,
                    slot: state.slot,
                    has_conflicts: state.has_conflicts,
                    is_stale: state.is_stale,
                    base_y,
                    parent_branch: parent,
                }
            })
            .collect();

        (result, branch_infos)
    }

    /// Look up merge positions from positioned commits (not fixed lanes).
    #[allow(clippy::unused_self)]
    pub fn position_merges_dynamic(
        &self,
        data: &CollectedData,
        positioned_commits: &[PositionedCommit],
    ) -> Vec<PositionedMerge> {
        // Build sha -> index into positioned_commits
        let sha_to_idx: HashMap<&str, usize> = positioned_commits
            .iter()
            .enumerate()
            .map(|(i, pc)| (pc.commit.sha.as_str(), i))
            .collect();

        // For each merge, find the last commit on from_branch before the merge commit,
        // and the merge commit itself.
        data.merges
            .iter()
            .filter_map(|m| {
                let merge_idx = sha_to_idx.get(m.sha.as_str())?;
                let merge_pc = &positioned_commits[*merge_idx];

                // Find the last commit on from_branch that appears before this merge
                let from_pc = positioned_commits[..*merge_idx]
                    .iter()
                    .rev()
                    .find(|pc| pc.commit.branch == m.from_branch)?;

                Some(PositionedMerge {
                    from_x: from_pc.x,
                    from_y: from_pc.y,
                    to_x: merge_pc.x,
                    to_y: merge_pc.y,
                    slot: from_pc.slot,
                    has_conflicts: from_pc.branch_has_conflicts,
                    is_stale: from_pc.branch_is_stale,
                })
            })
            .collect()
    }

    /// Create vertical markers for tagged default-branch commits.
    pub fn position_tags(&self, positioned_commits: &[PositionedCommit]) -> Vec<PositionedTag> {
        positioned_commits
            .iter()
            .filter(|pc| pc.is_default_branch && !pc.commit.tags.is_empty())
            .flat_map(|pc| {
                pc.commit.tags.iter().map(move |tag| {
                    let display = if tag.len() > 16 {
                        format!("{}...", &tag[..13])
                    } else {
                        tag.clone()
                    };
                    PositionedTag {
                        x: pc.x,
                        main_y: self.main_y,
                        label_y: self.main_y - 50.0,
                        tag_name: display,
                    }
                })
            })
            .collect()
    }

    /// Compute branch labels: the first commit position for each branch (including default).
    #[allow(clippy::unused_self)]
    pub fn compute_branch_labels(&self, positioned: &[PositionedCommit<'_>]) -> Vec<BranchLabel> {
        let mut seen: HashMap<String, bool> = HashMap::new();
        let mut labels = Vec::new();

        for pc in positioned {
            if seen.contains_key(&pc.commit.branch) {
                continue;
            }
            seen.insert(pc.commit.branch.clone(), true);

            let display_name = if pc.commit.branch.len() > 24 {
                format!("{}...", &pc.commit.branch[..21])
            } else {
                pc.commit.branch.clone()
            };

            labels.push(BranchLabel {
                name: display_name,
                x: pc.x,
                y: pc.y,
                slot: pc.slot,
                has_conflicts: pc.branch_has_conflicts,
                is_stale: pc.branch_is_stale,
            });
        }

        labels
    }

    pub fn compute_date_ticks(&self, data: &CollectedData) -> Vec<DateTick> {
        if data.commits.is_empty() {
            return Vec::new();
        }

        let total = data.commits.len();
        let mut ticks = Vec::new();
        let mut last_month: Option<(i32, u32)> = None;

        for (i, commit) in data.commits.iter().enumerate() {
            let year = commit.timestamp.year();
            let month = commit.timestamp.month();
            let key = (year, month);

            if last_month != Some(key) {
                last_month = Some(key);
                let x = self.commit_to_x(i, total);
                let label = format!("{year}/{month:02}");
                ticks.push(DateTick { x, label });
            }
        }

        ticks
    }
}
