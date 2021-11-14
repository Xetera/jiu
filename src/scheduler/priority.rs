use num_traits::FromPrimitive;
use sqlx::types::BigDecimal;
use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use crate::{models::ScrapeHistory, scheduler::MIN_PRIORITY};

use super::MAX_PRIORITY;

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

const MAX_RESULT_CONTRIBUTION: u32 = 3;

impl Priority {
    /// Decide the next priority based on the the recent scrape history of the
    /// provider priority.
    /// This function specifically borrows self as the result is compared with self
    /// to detect change
    pub fn next(&self, history: &[ScrapeHistory]) -> Self {
        let n = history.len() as u32;
        if n == 0 {
            return Self {
                level: BigDecimal::from_f32(1f32).unwrap(),
            };
        }
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
        let raw_weights = (0..n).map(|x| (n - x - 1).pow(2));
        let sum_raw_weight: u32 = raw_weights.clone().sum();
        let weights = raw_weights.map(|x| x as f32 / sum_raw_weight as f32);
        let weight_sum = weights.clone().sum::<f32>();
        let z = weights.zip(history);
        let raw_weighted_average: f32 = z
            .map(|(a, b)| (a * b.result_count.min(MAX_RESULT_CONTRIBUTION) as f32))
            .sum();
        let weighted_average: f32 = (raw_weighted_average * weight_sum) / weight_sum as f32;
        let scaled = weighted_average * (MAX_PRIORITY - MIN_PRIORITY) + MIN_PRIORITY;
        let level = scaled.clamp(MIN_PRIORITY, MAX_PRIORITY);
        // let increases = history
        //     .into_iter()
        //     .enumerate()
        //     // .map(|(i, history)| history.result_count.min(MAX_RESULT_CONTRIBUTION) * (past_scrape_count - i as u32))
        //     .map(|(i, history)| history.result_count.min(MAX_RESULT_CONTRIBUTION))
        //     .sum::<u32>();

        // let level =
        //     (increases / past_scrape_count) as f32 * (MAX_PRIORITY - MIN_PRIORITY) + MIN_PRIORITY;
        Self {
            level: BigDecimal::from_f32(level).unwrap(),
        }
        // Self {
        //     level: level.try_into().unwrap(),
        // }
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use num_traits::FromPrimitive;
    use sqlx::types::BigDecimal;

    use crate::{
        models::ScrapeHistory,
        scheduler::{MAX_PRIORITY, MIN_PRIORITY},
        scraper::ScopedProvider,
    };

    use super::Priority;

    #[test]
    fn priority_check() {
        let prio = Priority::unchecked_clamp(0f32);
        let make_hist = |count| ScrapeHistory {
            date: NaiveDateTime::from_timestamp(0, 0),
            priority: prio.clone(),
            provider: ScopedProvider {
                destination: "".to_owned(),
                name: crate::scraper::AllProviders::PinterestBoardFeed,
            },
            result_count: count,
        };
        let hist = make_hist(1);
        let n = prio.next(&[hist.clone(), hist.clone(), hist.clone(), hist.clone()]);
        assert_eq!(n.level, BigDecimal::from_f32(MAX_PRIORITY).unwrap());

        let n = prio.next(&(0..15).map(|_| hist.clone()).collect::<Vec<_>>());
        assert_eq!(n.level, BigDecimal::from_f32(MAX_PRIORITY).unwrap());

        let n = prio.next(&(0..15).map(|_| make_hist(0)).collect::<Vec<_>>());
        assert_eq!(n.level, BigDecimal::from_f32(MIN_PRIORITY).unwrap())
    }
}
