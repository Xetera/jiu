use crate::models::ScrapeHistory;
use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

#[derive(Debug)]
pub struct InvalidPriority(i32);

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Priority {
    level: i32,
    duration: Duration,
}

const MIN_LEVEL: i32 = 1;
const MAX_LEVEL: i32 = 10;

const HOURS_BY_SECS: u64 = 60 * 60;

const fn hours(num: u64) -> Duration {
    Duration::from_secs(HOURS_BY_SECS * num)
}

const fn days(num: u64) -> Duration {
    Duration::from_secs(HOURS_BY_SECS * 24 * num)
}

fn level_to_duration(level: i32) -> Duration {
    match level {
        // updated very frequently
        MIN_LEVEL => hours(2),
        2 => hours(8),
        3 => hours(12),
        4 => hours(18),
        5 => hours(24),
        6 => hours(36),
        7 => days(2),
        8 => days(4),
        9 => days(5),
        // updated very infrequently
        MAX_LEVEL => days(7),
        ffs @ _ => panic!("You were being lazy and didn't feel like wrapping priority levels in a newtype and now {} came back to bite you in the ass, good job idiot", ffs),
    }
}

impl TryFrom<i32> for Priority {
    type Error = InvalidPriority;
    fn try_from(level: i32) -> Result<Self, Self::Error> {
        if level < MIN_LEVEL || level > MAX_LEVEL {
            return Err(InvalidPriority(level));
        };
        Ok(Self {
            level,
            duration: level_to_duration(level),
        })
    }
}

impl From<Priority> for i32 {
    fn from(priority: Priority) -> Self {
        priority.level.try_into().unwrap()
    }
}

enum PriorityChange {
    Up,
    Down,
}

impl Priority {
    fn change(&self, history: &[ScrapeHistory]) -> Option<PriorityChange> {
        let same_priority_scrapes = history
            .iter()
            // the history could've jumped between 2 priorities within the same ScrapeHistory.
            // We only need the last chain of similar scrapes
            // For example: [A, A, A, B, B, A, A]
            // should result in
            // [A, A, A]
            .take_while(|history| history.priority.level == self.level)
            .collect::<Vec<&ScrapeHistory>>();
        let past_scrape_counts = same_priority_scrapes.len();
        let increases = same_priority_scrapes
            .into_iter()
            .filter(|history| history.result_count > 0)
            .collect::<Vec<&ScrapeHistory>>()
            .len();
        if increases > 0 {
            // Any new result within the same priority level should result in a priority increase
            return Some(PriorityChange::Up);
        } else if past_scrape_counts >= 3 {
            return Some(PriorityChange::Down);
        } else {
            None
        }
    }
    pub fn unchecked_clamp(level: i32) -> Self {
        level
            .clamp(MIN_LEVEL, MIN_LEVEL)
            .try_into()
            // something has gone very wrong if the level is out of bounds
            .expect(&format!("{} is not a valid priority", level))
    }
    /// Decide the next priority based on the the recent scrape history of the
    /// provider priority.
    /// This function specifically borrows self as the result is compared with self
    /// to detect change
    pub fn next(&self, history: &[ScrapeHistory]) -> Self {
        let level = self.level;
        match self.change(history) {
            None => *self,
            Some(PriorityChange::Up) => Priority::unchecked_clamp(level + 1),
            Some(PriorityChange::Down) => Priority::unchecked_clamp(level - 1),
        }
    }
}

pub fn update_priority() {}
