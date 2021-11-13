use crate::{
    db::Database,
    models::{PendingProvider, ScrapeHistory},
    scheduler::Priority,
    scraper::{AllProviders, ScopedProvider},
};
use chrono::{DateTime, Utc};
use itertools::{unfold, Itertools};
use num_traits::cast::{FromPrimitive, ToPrimitive};
use parking_lot::RwLock;
use rand::Rng;
use sqlx::types::BigDecimal;
use std::ops::{Add, Div, Sub};
use std::time::Duration;
use std::{collections::HashSet, convert::TryInto, hash::Hash, iter::FromIterator, str::FromStr};

pub type RunningProviders = HashSet<ScopedProvider>;

/// Scheduled providers are ready to be processed
#[derive(Debug)]
pub struct ScheduledProviders(Vec<PendingProvider>);

impl ScheduledProviders {
    pub fn providers(&self) -> &Vec<PendingProvider> {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Get a list of sorted scrapes that need to happen for that day
pub async fn pending_scrapes(db: &Database) -> anyhow::Result<Vec<PendingProvider>> {
    // all future scrapes that are specifically grouped by their provider name first
    let potential_target_providers = sqlx::query!(
        "SELECT * FROM provider_resource pr
        WHERE pr.enabled AND pr.tokens >= 1
        ORDER BY pr.name DESC, pr.destination desc"
    )
    .fetch_all(db)
    .await?;

    let groups = potential_target_providers
        .into_iter()
        .flat_map(|row| {
            let tokens = row.tokens.to_f32().unwrap().trunc() as i32;
            (0..tokens)
                .map(|_| {
                    (
                        row.id,
                        Priority::unchecked_clamp(row.priority.to_f32().unwrap()),
                        ScopedProvider {
                            destination: row.destination.clone(),
                            name: AllProviders::from_str(&row.name).unwrap(),
                        }, // last_scrape: row.last_scrape,
                        row.last_scrape,
                    )
                })
                .collect::<Vec<_>>()
        })
        .group_by(|p| p.2.name);

    let out = groups
        .into_iter()
        .flat_map(|(_, group)| {
            let endpoints = group.collect::<Vec<_>>();
            let maximized_endpoints = maximize_distance(&endpoints, quality_maxmindist);
            let dates = interpolate_dates(
                maximized_endpoints.len(),
                // We want to give the
                &Duration::from_millis(1000 * 60),
                // One day
                &Duration::from_millis(8.64e7 as u64),
            );
            maximized_endpoints
                .iter()
                .zip(dates)
                .map(
                    |((id, priority, provider, last_scrape), scrape_date)| PendingProvider {
                        id: *id,
                        priority: priority.clone(),
                        provider: provider.clone(),
                        scrape_date,
                        last_scrape: *last_scrape,
                    },
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    Ok(out)
}

pub async fn mark_as_scheduled(
    db: &Database,
    pp: &PendingProvider,
    running_providers: &RwLock<RunningProviders>,
) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE provider_resource SET last_queue = NOW() WHERE id = $1",
        pp.id
    )
    .fetch_optional(db)
    .await?;
    let mut handle = running_providers.write();
    handle.insert(pp.provider.clone());
    Ok(())
}

/// Vec length is equal to the length of the items passed in
fn interpolate_dates(
    item_count: usize,
    start_duration: &Duration,
    end_duration: &Duration,
) -> Vec<Duration> {
    println!("{:?}, {:?}", start_duration, end_duration);
    let duration = *end_duration - *start_duration;
    println!("{:?}", duration);
    let initial_gap = duration.checked_div(item_count as u32 + 1).unwrap();
    println!("{:?}", initial_gap);
    unfold(*start_duration, |duration| {
        let next = duration.add(initial_gap);
        *duration = next;
        Some(next)
    })
    .by_ref()
    .take(item_count)
    .collect::<Vec<_>>()
}

pub async fn update_priorities(db: &Database, sp: &Vec<PendingProvider>) -> anyhow::Result<()> {
    let providers = sqlx::query!(
        "SELECT
            pr.id,
            pr.name,
            pr.destination,
            s.priority as resource_priority,
            s.scraped_at,
            s.priority,
            (SELECT COUNT(*)
              FROM media m
              INNER JOIN scrape_request sr
                on sr.id = m.scrape_request_id
              where sr.scrape_id = s.id
            ) as discovery_count
        FROM provider_resource pr
        LEFT JOIN LATERAL (
            SELECT *
            FROM scrape s
            WHERE s.provider_name = pr.name
              AND s.provider_destination = pr.destination
            ORDER BY s.scraped_at desc, id
            LIMIT 30
        ) s on True
        WHERE pr.enabled AND pr.id = ANY($1)
        ORDER BY s.scraped_at desc",
        &sp.iter().map(|pp| pp.id).collect::<Vec<_>>()
    )
    .fetch_all(db)
    .await?;

    let groups = providers.iter().group_by(|row| {
        (
            row.id,
            row.name.clone(),
            row.destination.clone(),
            row.resource_priority.clone(),
        )
    });
    for ((id, name, destination, resource_priority), rows) in &groups {
        let histories = rows
            .filter(|row| row.scraped_at.is_some())
            .map(|row| ScrapeHistory {
                date: row.scraped_at.unwrap(),
                priority: Priority::unchecked_clamp(row.priority.to_f32().unwrap()),
                result_count: row.discovery_count.unwrap_or(0i64).try_into().unwrap(),
                provider: ScopedProvider {
                    destination: destination.clone(),
                    name: AllProviders::from_str(&name).unwrap(),
                },
            })
            .collect::<Vec<ScrapeHistory>>();

        let provider_priority = Priority::unchecked_clamp(resource_priority.to_f32().unwrap());
        let next_priority = provider_priority.next(&histories[..]);
        sqlx::query!(
            "UPDATE provider_resource SET priority = $1 where id = $2 returning id",
            next_priority.level,
            id
        )
        .fetch_one(db)
        .await?;
    }
    // Update tokens for all resources. This has to be run after priorities are
    // updated
    sqlx::query!(
        "UPDATE provider_resource
        SET
            tokens = tokens + priority,
            last_token_update = NOW()
        WHERE last_token_update IS NULL OR last_token_update + interval '1 day' <= NOW()"
    )
    .fetch_one(db)
    .await?;
    Ok(())
}

pub fn maximize_distance<T: Hash + Eq + Clone>(items: &Vec<T>, quality: fn(&[T]) -> f32) -> Vec<T> {
    let mut out = items.clone();
    let mut no_improvement = 0;
    let mut best = 0f32;
    let mut rng = rand::thread_rng();
    while no_improvement < 400 {
        let i = rng.gen_range(0..out.len());
        let j = rng.gen_range(0..out.len());
        let mut copy = out.clone();
        copy.swap(i, j);
        let q = quality(&copy);
        if q > best {
            out = copy;
            best = q;
            no_improvement = 0;
        } else {
            no_improvement += 1;
        }
    }
    out
}

fn quality_maxmindist<T: Hash + Eq>(items: &[T]) -> f32 {
    let mut s = 0f32;
    let uniq: HashSet<&T> = HashSet::from_iter(items);
    for item in uniq.into_iter() {
        let indices = (0..items.len())
            .filter_map(|i| {
                if &items[i] == item {
                    Some(i as i32)
                } else {
                    None
                }
            })
            .collect::<Vec<i32>>();
        if indices.len() > 1 {
            let summed: f32 = (0..indices.len() - 1)
                .map(|i| 1f32 / (indices[i + 1] - indices[i]) as f32)
                .sum();
            s += summed;
        }
    }
    1f32 / s
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::scheduler::scheduler::quality_maxmindist;

    use super::{interpolate_dates, maximize_distance};

    #[test]
    fn spacing_test() {
        assert_eq!(
            maximize_distance(&vec![1, 1, 1, 2, 2], quality_maxmindist),
            &[1, 2, 1, 2, 1],
        );
        // this algorithm is so bad I can't even get
        // anything over a couple element to work lmfaooo consistently

        // assert_eq!(
        //     maximize_distance(
        //         &vec![1, 1, 1, 1, 1, 2, 2, 2, 3, 3, 4, 4, 5, 6],
        //         quality_maxmindist
        //     ),
        //     &[1, 2, 3, 1, 4, 5, 1, 2, 6, 3, 1, 4, 2, 1]
        // );
    }

    #[test]
    fn interpolate() {
        let out: Vec<Duration> =
            interpolate_dates(3, &Duration::from_millis(0), &Duration::from_millis(3000));
        let res: Vec<Duration> = vec![
            Duration::from_millis(750),
            Duration::from_millis(1500),
            Duration::from_millis(2250),
        ];
        assert_eq!(out, res)
    }
}
