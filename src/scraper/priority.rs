use crate::models::ScrapeHistory;
use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

#[derive(Debug)]
pub struct InvalidPriority(u32);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Priority {
    level: u32,
    duration: Duration,
}

const MIN_LEVEL: u32 = 1;
const MAX_LEVEL: u32 = 10;

const HOURS_BY_MILLIS: u64 = 1000 * 60 * 60;

const fn hours(num: u64) -> Duration {
    Duration::from_millis(HOURS_BY_MILLIS * num)
}

const fn days(num: u64) -> Duration {
    Duration::from_millis(HOURS_BY_MILLIS * 24 * num)
}

impl TryFrom<u32> for Priority {
    type Error = InvalidPriority;
    fn try_from(level: u32) -> Result<Self, Self::Error> {
        if level < MIN_LEVEL || level > MAX_LEVEL {
            return Err(InvalidPriority(level));
        };
        Ok(Self {
            level,
            duration: match level {
                // updated very frequently
                MIN_LEVEL => hours(4),
                2 => hours(8),
                3 => hours(12),
                4 => hours(18),
                5 => days(1),
                6 => days(2),
                7 => days(3),
                8 => days(4),
                9 => days(5),
                // updated very infrequently
                MAX_LEVEL => days(7),
                ffs @ _ => panic!("You were being lazy and didn't feel like wrapping priority levels in a newtype and now {} came back to bite you in the ass, good job idiot", ffs),
            },
        })
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
            .filter(|history| history.priority.level == self.level)
            .collect::<Vec<&ScrapeHistory>>();
        let past_scrape_counts = same_priority_scrapes.len();
        let increases = same_priority_scrapes
            .into_iter()
            .filter(|history| history.result_count > 0)
            .collect::<Vec<&ScrapeHistory>>()
            .len();
        if increases > 0 {
            // Any amount of increase in the same scrape should result in a priority increase
            return Some(PriorityChange::Up);
        } else if past_scrape_counts >= 3 {
            return Some(PriorityChange::Down);
        } else {
            None
        }
    }
    fn unchecked_clamp(level: u32) -> Self {
        level
            .clamp(MIN_LEVEL, MIN_LEVEL)
            .try_into()
            .expect(&format!("{} is not a valid priority", level))
    }
    pub fn next(&self, history: &[ScrapeHistory]) -> Self {
        match self.change(history) {
            None => self.clone(),
            // something has gone very wrong if the level is
            Some(PriorityChange::Up) => Priority::unchecked_clamp(self.level + 1),
            Some(PriorityChange::Down) => Priority::unchecked_clamp(self.level - 1),
        }
    }
}
