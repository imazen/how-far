#![doc = include_str!("../README.md")]
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;
use how_far_along::{NodeId, Snapshot, Status, Total};

/// Smooths a polled tree's fraction between reports, for display; see the
/// [crate docs](crate). Keep one per observed tree of one operation and pass
/// it every snapshot of that tree you display.
#[derive(Default)]
pub struct ProgressSmoother {
    leaves: Vec<Pace>,
}

impl core::fmt::Debug for ProgressSmoother {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProgressSmoother")
            .field("tracked_paces", &self.leaves.len())
            .finish()
    }
}

/// What one running leaf's reports have looked like so far.
struct Pace {
    node: NodeId,
    /// The total the pace was learned against; a revision starts over.
    total: u64,
    /// The latest count, and when the change to it was first seen.
    count: u64,
    since: Duration,
    /// Units per change and seconds per change, averaged over the changes seen.
    step: f64,
    interval: f64,
    changes: u32,
    /// The latest change, which bounds how far the display runs ahead.
    last_step: f64,
    /// The latest change's starting count and gap, and the averages before
    /// it, so that a change seen at the same time can join it.
    last_base: u64,
    last_interval: f64,
    before: (f64, f64, u32),
    /// The largest fraction shown, so a running leaf's display never goes back.
    shown: f64,
    /// Seen in the current call; leaves no longer running are dropped.
    seen: bool,
}

impl Pace {
    fn new(node: NodeId, total: u64, count: u64, now: Duration) -> Self {
        Self {
            node,
            total,
            count,
            since: now,
            step: 0.0,
            interval: 0.0,
            changes: 0,
            last_step: 0.0,
            last_base: count,
            last_interval: 0.0,
            before: (0.0, 0.0, 0),
            shown: 0.0,
            seen: true,
        }
    }

    /// Fold one change of `step` units over `interval` seconds into the pace.
    fn learn(&mut self, step: f64, interval: f64) {
        // The first change, or a shorter gap, sets the pace; a longer gap
        // moves it a quarter of the way, so one stall doesn't slow it for
        // many reports.
        let weight = if self.changes == 0 || interval < self.interval {
            1.0
        } else {
            0.25
        };
        self.step += (step - self.step) * weight;
        self.interval += (interval - self.interval) * weight;
        self.changes = self.changes.saturating_add(1);
        self.last_step = step;
    }

    /// Take a new count, then estimate the fraction of `total` done at `now`.
    fn estimate(&mut self, count: u64, now: Duration) -> f64 {
        if count > self.count {
            let elapsed = now.saturating_sub(self.since).as_secs_f64();
            if elapsed > 0.0 {
                self.before = (self.step, self.interval, self.changes);
                self.last_base = self.count;
                self.last_interval = elapsed;
                self.learn((count - self.count) as f64, elapsed);
                self.since = now;
            } else if self.changes != 0 {
                // Seen at the same time as the latest change (or, with a
                // clock that went back, earlier): it is part of that change.
                (self.step, self.interval, self.changes) = self.before;
                self.learn((count - self.last_base) as f64, self.last_interval);
            }
            // Without a change seen yet, a count seen at the first poll's
            // time is the starting count.
            self.count = count;
        }
        let total = self.total as f64;
        let mut estimate = self.count as f64;
        if self.count < self.total && self.changes != 0 && self.interval > 0.0 {
            let elapsed = now.saturating_sub(self.since).as_secs_f64();
            // At most the latest change ahead, and at most half the work
            // left, so extrapolation alone never completes a stage.
            estimate += (self.step * elapsed / self.interval)
                .min(self.step)
                .min(self.last_step)
                .min((total - estimate) / 2.0);
        }
        let fraction = (estimate / total).min(1.0);
        self.shown = self.shown.max(fraction);
        self.shown
    }
}

impl ProgressSmoother {
    /// A smoother that has seen no reports.
    pub fn new() -> Self {
        Self::default()
    }

    /// The fraction of `snapshot` to display at `now`: running leaves move
    /// at the pace of their reports so far, and everything else is the
    /// snapshot's own record. `None` when the snapshot's own fraction is.
    ///
    /// `now` comes from any monotonic clock of the caller's, as a time since
    /// an epoch of its choosing; pass the same clock on every call. A count
    /// change seen at the same time as the previous one is part of that
    /// change, and an earlier time counts as the same time. Calling again
    /// with the same snapshot and a later time moves the display on.
    ///
    /// While a leaf runs under the same total its display never goes back.
    /// A finished leaf shows what the snapshot records, and a revised total
    /// starts the leaf over from its count, so the display steps back when
    /// a stage fails, is cancelled, or has its total raised.
    pub fn display_fraction(&mut self, snapshot: &Snapshot, now: Duration) -> Option<f64> {
        for pace in &mut self.leaves {
            pace.seen = false;
        }
        let estimate = self.node(snapshot, now);
        // Forget leaves that are no longer running; the rest stay sorted.
        self.leaves.retain(|pace| pace.seen);
        estimate
    }

    /// [`Snapshot::fraction`], with running leaves estimated.
    fn node(&mut self, node: &Snapshot, now: Duration) -> Option<f64> {
        if node.children.is_empty() {
            return match (node.status, node.total) {
                (Status::Running, Total::Exact(total) | Total::Estimated(total))
                    if total != 0 && !node.overflowed =>
                {
                    Some(self.leaf(node.id, node.completed, total, now))
                }
                _ => node.fraction(),
            };
        }
        if let Status::Finished(_) = node.status {
            return node.fraction();
        }
        let mut sum = 0.0;
        for child in &node.children {
            sum += child.weight as f64;
        }
        let (mut result, mut known) = (0.0, true);
        // Visit every child, so that no running leaf loses its pace.
        for child in &node.children {
            match self.node(child, now) {
                Some(fraction) => result += child.weight as f64 / sum * fraction,
                None => known = false,
            }
        }
        known.then_some(result.clamp(0.0, 1.0))
    }

    fn leaf(&mut self, node: NodeId, count: u64, total: u64, now: Duration) -> f64 {
        // Sorted by node: a lookup costs O(log leaves); a leaf seen for the
        // first time is inserted, shifting the leaves after it.
        let index = match self.leaves.binary_search_by_key(&node, |pace| pace.node) {
            Ok(index) => index,
            Err(index) => {
                self.leaves
                    .insert(index, Pace::new(node, total, count, now));
                index
            }
        };
        let pace = &mut self.leaves[index];
        if pace.total != total {
            *pace = Pace::new(node, total, count, now);
        }
        pace.seen = true;
        pace.estimate(count, now)
    }
}
