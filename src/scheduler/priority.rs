use num_traits::FromPrimitive;
use sqlx::types::BigDecimal;
use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use crate::models::ScrapeHistory;

#[derive(Debug)]
pub struct InvalidPriority(f32);

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Hash)]
pub struct Priority {
    pub level: BigDecimal,
}

const HOURS_BY_SECS: u64 = 60 * 60;

const fn hours(num: u64) -> Duration {
    Duration::from_secs(HOURS_BY_SECS * num)
}

const fn days(num: u64) -> Duration {
    Duration::from_secs(HOURS_BY_SECS * 24 * num)
}

impl From<f32> for Priority {
    fn from(level: f32) -> Self {
        Self {
            level: BigDecimal::from_f32(level).unwrap(),
        }
    }
}

enum PriorityChange {
    Up,
    Down,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::unchecked_clamp(1f32)
    }
}

// fn priority_weights(target: i32){
//     (0..).map(|num|)
// }

const MAX_RESULT_CONTRIBUTION: u32 = 3;

const MIN_PRIORITY: f32 = 0.07;
const MAX_PRIORITY: f32 = 1.75;

impl Priority {
    /// Decide the next priority based on the the recent scrape history of the
    /// provider priority.
    /// This function specifically borrows self as the result is compared with self
    /// to detect change
    pub fn next(&self, history: &[ScrapeHistory]) -> Self {
        let past_scrape_count = history.len() as u32;
        // let same_priority_scrapes = history
        //     .iter()
        //     // the history could've jumped between 2 priorities within the same ScrapeHistory.
        //     // We only need the last chain of similar scrapes
        //     // For example: [A, A, A, B, B, A, A]
        //     // should result in
        //     // [A, A, A]
        //     .take_while(|history| history.priority.level == self.level)
        //     .collect::<Vec<&ScrapeHistory>>();
        // let past_scrape_counts = same_priority_scrapes.len();
        let increases = history
            .into_iter()
            .enumerate()
            // .map(|(i, history)| history.result_count.min(MAX_RESULT_CONTRIBUTION) * (past_scrape_count - i as u32))
            .map(|(i, history)| history.result_count.min(MAX_RESULT_CONTRIBUTION))
            .sum::<u32>();

        let level =
            (increases / past_scrape_count) as f32 * (MAX_PRIORITY - MIN_PRIORITY) + MIN_PRIORITY;
        Self {
            level: level.try_into().unwrap(),
        }
        // // we want the amount of allowed empty scrapes to scale inversely with level
        // // so faster scrape rates have more leeway before they stop dropping down in levels
        // let expected_empty_scrapes = (((MAX_LEVEL + 1f32) - self.level) * 1.2).floor() as usize;
        // if increases > 0 {
        //     // Any new result within the same priority level should result in a priority increase
        //     return Some(PriorityChange::Up);
        // } else if past_scrape_counts >= expected_empty_scrapes {
        //     return Some(PriorityChange::Down);
        // } else {
        //     None
        // }
    }
    pub fn unchecked_clamp(level: f32) -> Self {
        level
            .clamp(MIN_PRIORITY, MAX_PRIORITY)
            .try_into()
            // something has gone very wrong if the level is out of bounds
            .expect(&format!("{} is not a valid priority", level))
    }
}
