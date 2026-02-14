#![allow(dead_code)]
use crate::data::{CollectedData, Commit};
use chrono::{DateTime, Utc};

/// A positioned commit ready for rendering.
pub struct PositionedCommit<'a> {
    pub commit: &'a Commit,
    pub x: f32,
    pub y: f32,
    pub lane: usize,
}

/// A positioned merge line.
pub struct PositionedMerge {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
}

/// Layout configuration.
pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub margin: f32,
    pub lane_height: f32,
    pub branch_lanes: Vec<(String, usize)>,
    pub time_start: DateTime<Utc>,
    pub time_end: DateTime<Utc>,
}

impl Layout {
    pub fn from_data(data: &CollectedData, width: u32, height: u32) -> Self {
        let margin = 80.0;
        let lane_height = 60.0;

        // Assign lanes: default branch at top, others below
        let mut branch_lanes: Vec<(String, usize)> = Vec::new();
        let mut lane = 0;

        // Default branch first
        for b in &data.branches {
            if b.is_default {
                branch_lanes.push((b.name.clone(), lane));
                lane += 1;
                break;
            }
        }

        // Then non-default branches
        for b in &data.branches {
            if !b.is_default {
                branch_lanes.push((b.name.clone(), lane));
                lane += 1;
            }
        }

        // Time range from commits
        let time_start = data
            .commits
            .iter()
            .map(|c| c.timestamp)
            .min()
            .unwrap_or_else(Utc::now);
        let time_end = data
            .commits
            .iter()
            .map(|c| c.timestamp)
            .max()
            .unwrap_or_else(Utc::now);

        Layout {
            width,
            height,
            margin,
            lane_height,
            branch_lanes,
            time_start,
            time_end,
        }
    }

    /// Map a timestamp to an X position.
    fn time_to_x(&self, ts: &DateTime<Utc>) -> f32 {
        let total_duration = (self.time_end - self.time_start).num_seconds().max(1) as f32;
        let elapsed = (*ts - self.time_start).num_seconds() as f32;
        let usable_width = self.width as f32 - 2.0 * self.margin;
        self.margin + (elapsed / total_duration) * usable_width
    }

    /// Map a branch name to a Y position.
    fn branch_to_y(&self, branch: &str) -> (f32, usize) {
        for (name, lane) in &self.branch_lanes {
            if name == branch {
                let y = self.margin + (*lane as f32) * self.lane_height + self.lane_height / 2.0;
                return (y, *lane);
            }
        }
        // Unknown branch: put at bottom
        let y = self.margin
            + (self.branch_lanes.len() as f32) * self.lane_height
            + self.lane_height / 2.0;
        (y, self.branch_lanes.len())
    }

    /// Position all commits.
    pub fn position_commits<'a>(&self, data: &'a CollectedData) -> Vec<PositionedCommit<'a>> {
        data.commits
            .iter()
            .map(|c| {
                let x = self.time_to_x(&c.timestamp);
                let (y, lane) = self.branch_to_y(&c.branch);
                PositionedCommit {
                    commit: c,
                    x,
                    y,
                    lane,
                }
            })
            .collect()
    }

    /// Position merge lines.
    pub fn position_merges(&self, data: &CollectedData) -> Vec<PositionedMerge> {
        data.merges
            .iter()
            .map(|m| {
                let x = self.time_to_x(&m.timestamp);
                let (from_y, _) = self.branch_to_y(&m.from_branch);
                let (to_y, _) = self.branch_to_y(&m.to_branch);
                PositionedMerge {
                    from_x: x,
                    from_y,
                    to_x: x,
                    to_y,
                }
            })
            .collect()
    }
}
