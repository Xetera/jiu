use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use num_traits::FromPrimitive;
use sqlx::types::BigDecimal;

use crate::{models::ScrapeHistory, scheduler::MIN_PRIORITY};

use super::MAX_PRIORITY;

#[derive(Debug)]
pub struct InvalidPriority(f32);

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Hash)]
pub struct Priority {
    pub level: BigDecimal,
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
    pub fn next(&self, history: &[ScrapeHistory]) -> Self {
        if history.is_empty() {
            return Self {
                level: BigDecimal::from_f32(1f32).unwrap(),
            };
        }
        let n = history.len() as i32;

        let raw_weights = (0i32..n).map(|x| (x - n - 1).pow(2));
        let sum_raw_weight: i32 = raw_weights.clone().sum();
        let weights = raw_weights.map(|x| x as f32 / sum_raw_weight as f32);
        let weight_sum = weights.clone().sum::<f32>();
        let z = weights.zip(history);
        let raw_weighted_average: f32 = z
            .map(|(a, b)| (a * b.result_count.min(MAX_RESULT_CONTRIBUTION) as f32))
            .sum();

        let weighted_average: f32 = (raw_weighted_average * weight_sum) / weight_sum as f32;
        let scaled = weighted_average * (MAX_PRIORITY - MIN_PRIORITY) + MIN_PRIORITY;
        let level = scaled.clamp(MIN_PRIORITY, MAX_PRIORITY);

        // in some strange situations f32 is NaN.
        // These cases are normally handled at the top of the function but if not... we just default to
        // the existing thing
        let level = BigDecimal::from_f32(level).unwrap_or_else(|| self.level.clone());
        Self { level }
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

        let n = prio.next(&(0..15).map(|_| make_hist(1)).collect::<Vec<_>>());
        assert_eq!(n.level, BigDecimal::from_f32(MAX_PRIORITY).unwrap());

        let n = prio.next(&(0..15).map(|_| make_hist(0)).collect::<Vec<_>>());
        assert_eq!(n.level, BigDecimal::from_f32(MIN_PRIORITY).unwrap())
    }
}
