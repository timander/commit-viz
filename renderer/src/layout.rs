use crate::data::{CollectedData, Commit};
use chrono::Datelike;
use std::collections::HashMap;

pub struct PositionedCommit<'a> {
    pub commit: &'a Commit,
    pub x: f32,
    pub y: f32,
    pub lane: usize,
    pub radius: f32,
    /// Width of the commit rectangle (proportional to files_changed)
    pub rect_w: f32,
    /// Height of the commit rectangle (proportional to lines changed)
    pub rect_h: f32,
    /// Whether this is on the default (sacred timeline) branch
    pub is_default_branch: bool,
}

pub struct PositionedMerge {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub lane: usize,
    pub from_branch: String,
}

pub struct DateTick {
    pub x: f32,
    pub label: String,
}

/// Tracks when a branch first appears and where it should be labeled
pub struct BranchLabel {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub lane: usize,
}

pub struct NetworkLayout {
    pub width: u32,
    pub height: u32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub lane_height: f32,
    pub branch_lanes: HashMap<String, usize>,
    pub default_branch: String,
    pub total_lanes: usize,
}

const MIN_RECT_W: f32 = 4.0;
const MAX_RECT_W: f32 = 20.0;
const MIN_RECT_H: f32 = 4.0;
const MAX_RECT_H: f32 = 24.0;
const MIN_RADIUS: f32 = 3.0;
const MAX_RADIUS: f32 = 12.0;

impl NetworkLayout {
    pub fn from_data(data: &CollectedData, width: u32, height: u32) -> Self {
        let margin_top = 70.0;
        let margin_bottom = 120.0;
        let margin_left = 80.0;
        let margin_right = 40.0;

        // Find default branch
        let default_branch = data
            .branches
            .iter()
            .find(|b| b.is_default)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "main".to_string());

        // Assign lanes only to branches that have commits
        let mut branch_commit_counts: HashMap<String, usize> = HashMap::new();
        for c in &data.commits {
            *branch_commit_counts.entry(c.branch.clone()).or_insert(0) += 1;
        }

        // Default branch centered, others distributed above/below
        let mut active_branches: Vec<String> = branch_commit_counts
            .keys()
            .filter(|b| *b != &default_branch)
            .cloned()
            .collect();
        active_branches.sort();

        // Limit lanes to avoid overcrowding
        let max_visible = ((height as f32 - margin_top - margin_bottom) / 40.0) as usize;
        if active_branches.len() > max_visible.saturating_sub(1) {
            active_branches.sort_by(|a, b| {
                branch_commit_counts
                    .get(b)
                    .unwrap_or(&0)
                    .cmp(branch_commit_counts.get(a).unwrap_or(&0))
            });
            active_branches.truncate(max_visible.saturating_sub(1));
            active_branches.sort();
        }

        let total_lanes = active_branches.len() + 1;
        let usable_height = height as f32 - margin_top - margin_bottom;
        let lane_height = (usable_height / total_lanes as f32).min(60.0);

        // Center default branch, distribute others above/below
        let mut branch_lanes: HashMap<String, usize> = HashMap::new();
        let center = total_lanes / 2;
        branch_lanes.insert(default_branch.clone(), center);

        let mut above = (0..center).rev().collect::<Vec<_>>();
        let mut below = ((center + 1)..total_lanes).collect::<Vec<_>>();

        for (i, branch) in active_branches.iter().enumerate() {
            if i % 2 == 0 {
                if let Some(lane) = above.pop() {
                    branch_lanes.insert(branch.clone(), lane);
                } else if let Some(lane) = below.pop() {
                    branch_lanes.insert(branch.clone(), lane);
                }
            } else if let Some(lane) = below.pop() {
                branch_lanes.insert(branch.clone(), lane);
            } else if let Some(lane) = above.pop() {
                branch_lanes.insert(branch.clone(), lane);
            }
        }

        NetworkLayout {
            width,
            height,
            margin_left,
            margin_right,
            margin_top,
            margin_bottom,
            lane_height,
            branch_lanes,
            default_branch,
            total_lanes,
        }
    }

    fn commit_to_x(&self, index: usize, total: usize) -> f32 {
        let usable = self.width as f32 - self.margin_left - self.margin_right;
        if total <= 1 {
            return self.margin_left + usable / 2.0;
        }
        self.margin_left + (index as f32 / (total - 1) as f32) * usable
    }

    fn branch_to_y(&self, branch: &str) -> (f32, usize) {
        let lane = self
            .branch_lanes
            .get(branch)
            .copied()
            .unwrap_or(self.total_lanes);

        let y = self.margin_top + (lane as f32 + 0.5) * self.lane_height;
        (y, lane)
    }

    fn commit_radius(commit: &Commit) -> f32 {
        let changes = (commit.insertions + commit.deletions) as f32;
        if changes <= 0.0 {
            return MIN_RADIUS;
        }
        let scaled = (changes.ln() / 10.0_f32.ln()) * (MAX_RADIUS - MIN_RADIUS) + MIN_RADIUS;
        scaled.clamp(MIN_RADIUS, MAX_RADIUS)
    }

    /// Compute rectangular dimensions for a commit.
    /// Width proportional to files_changed (log scale).
    /// Height proportional to lines changed (log scale).
    fn commit_rect(commit: &Commit) -> (f32, f32) {
        let files = commit.files_changed.max(1) as f32;
        let lines = (commit.insertions + commit.deletions).max(1) as f32;

        let w = (files.ln() / 10.0_f32.ln()) * (MAX_RECT_W - MIN_RECT_W) + MIN_RECT_W;
        let h = (lines.ln() / 10.0_f32.ln()) * (MAX_RECT_H - MIN_RECT_H) + MIN_RECT_H;

        (w.clamp(MIN_RECT_W, MAX_RECT_W), h.clamp(MIN_RECT_H, MAX_RECT_H))
    }

    pub fn position_commits<'a>(&self, data: &'a CollectedData) -> Vec<PositionedCommit<'a>> {
        let total = data.commits.len();
        data.commits
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let x = self.commit_to_x(i, total);
                let (y, lane) = self.branch_to_y(&c.branch);
                let radius = Self::commit_radius(c);
                let (rect_w, rect_h) = Self::commit_rect(c);
                let is_default_branch = c.branch == self.default_branch;
                PositionedCommit {
                    commit: c,
                    x,
                    y,
                    lane,
                    radius,
                    rect_w,
                    rect_h,
                    is_default_branch,
                }
            })
            .collect()
    }

    pub fn position_merges(&self, data: &CollectedData) -> Vec<PositionedMerge> {
        let sha_to_idx: HashMap<&str, usize> = data
            .commits
            .iter()
            .enumerate()
            .map(|(i, c)| (c.sha.as_str(), i))
            .collect();

        let total = data.commits.len();

        data.merges
            .iter()
            .filter_map(|m| {
                let idx = sha_to_idx.get(m.sha.as_str())?;
                let x = self.commit_to_x(*idx, total);
                let (from_y, lane) = self.branch_to_y(&m.from_branch);
                let (to_y, _) = self.branch_to_y(&m.to_branch);
                Some(PositionedMerge {
                    from_x: x - 20.0,
                    from_y,
                    to_x: x,
                    to_y,
                    lane,
                    from_branch: m.from_branch.clone(),
                })
            })
            .collect()
    }

    /// Compute branch labels: the first commit position for each non-default branch
    pub fn compute_branch_labels<'a>(&self, positioned: &[PositionedCommit<'a>]) -> Vec<BranchLabel> {
        let mut seen: HashMap<String, bool> = HashMap::new();
        let mut labels = Vec::new();

        for pc in positioned {
            if pc.commit.branch == self.default_branch {
                continue;
            }
            if seen.contains_key(&pc.commit.branch) {
                continue;
            }
            seen.insert(pc.commit.branch.clone(), true);
            labels.push(BranchLabel {
                name: pc.commit.branch.clone(),
                x: pc.x,
                y: pc.y,
                lane: pc.lane,
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

            if last_month.map_or(true, |lm| lm != key) {
                last_month = Some(key);
                let x = self.commit_to_x(i, total);
                let label = format!("{}/{:02}", year, month);
                ticks.push(DateTick { x, label });
            }
        }

        ticks
    }
}
