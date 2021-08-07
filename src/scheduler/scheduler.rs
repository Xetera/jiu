use crate::{
    db::Database,
    models::{PendingProvider, ScrapeHistory},
    scheduler::Priority,
    scraper::{AllProviders, ScopedProvider},
};
use chrono::{DateTime, Duration, Utc};
use itertools::Itertools;
use parking_lot::RwLock;
use std::{collections::HashSet, convert::TryInto, str::FromStr};

pub type RunningProviders = HashSet<ScopedProvider>;

/// Scheduled providers are ready to be processed
#[derive(Debug)]
pub struct ScheduledProviders(Vec<PendingProvider>);

impl ScheduledProviders {
    pub fn providers(self) -> Vec<PendingProvider> {
        self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

pub async fn pending_scrapes(
    db: &Database,
    running_providers: &RwLock<RunningProviders>,
) -> anyhow::Result<Vec<PendingProvider>> {
    // we're assuming that any request that previously started is _probably_ finished by an
    // hour and a half because the smallest scrape interval is 2 hours.
    // this might not always hold true and in the case of a congested request queue
    // it might cause backpressure so it has to be checked manually before requeuing
    let potential_target_providers = sqlx::query!(
        "SELECT * FROM provider_resource pr
        WHERE pr.enabled AND
        (pr.last_queue IS NULL OR pr.last_queue + '1.5 hour' < NOW())
        ORDER BY pr.last_scrape desc nulls first, created_at desc
        LIMIT 5" // nulls first to make sure unscraped endpoints get scraped first
    )
    // the limit lets us distribute the scrapes over a duration of multiple minutes
    // when fetching data for the first time to avoid re-scheduling scrapes of similar priority
    // back to the exact same date
    .fetch_all(db)
    .await?;

    let running = running_providers.read();
    Ok(potential_target_providers
        .into_iter()
        .map(|row| PendingProvider {
            id: row.id,
            priority: Priority::unchecked_clamp(row.priority),
            provider: ScopedProvider {
                destination: row.destination,
                name: AllProviders::from_str(&row.name).unwrap(),
            },
            last_scrape: row.last_scrape,
        })
        .filter(|sp| !running.contains(&sp.provider))
        .collect::<Vec<_>>())
}

pub async fn mark_as_scheduled(
    db: &Database,
    pending_providers: &ScheduledProviders,
    running_providers: &RwLock<RunningProviders>,
) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE provider_resource SET last_queue = NOW() WHERE id = ANY($1)",
        &pending_providers
            .0
            .iter()
            .map(|pv| pv.id)
            .collect::<Vec<_>>(),
    )
    .fetch_optional(db)
    .await?;
    let mut handle = running_providers.write();
    handle.extend(pending_providers.0.iter().map(|pp| pp.provider.clone()));
    Ok(())
}

pub async fn update_priorities(
    db: &Database,
    pending_providers: &[PendingProvider],
) -> anyhow::Result<()> {
    let providers = sqlx::query!(
        "SELECT
            pr.id,
            pr.name,
            pr.destination,
            pr.priority as resource_priority,
            pr.last_scrape,
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
            ORDER BY last_scrape desc, id
            LIMIT 10
        ) s on True
        WHERE pr.enabled AND pr.id = ANY($1)",
        &pending_providers.iter().map(|pp| pp.id).collect::<Vec<_>>()
    )
    .fetch_all(db)
    .await?;

    let groups = providers.iter().group_by(|row| {
        (
            row.id,
            row.name.clone(),
            row.destination.clone(),
            row.resource_priority,
        )
    });
    for ((id, name, destination, resource_priority), rows) in &groups {
        // let a = rows.group_by(|a| a.ed).collect();
        let histories = rows
            .filter(|row| row.last_scrape.is_some())
            .map(|row| ScrapeHistory {
                date: row.last_scrape.unwrap(),
                priority: Priority::unchecked_clamp(row.priority.unwrap()),
                result_count: row.discovery_count.unwrap_or(0i64).try_into().unwrap(),
                provider: ScopedProvider {
                    destination: destination.clone(),
                    name: AllProviders::from_str(&name).unwrap(),
                },
            })
            .collect::<Vec<ScrapeHistory>>();

        let provider_priority = Priority::unchecked_clamp(resource_priority);
        let next_priority = provider_priority.next(&histories[..]);
        if provider_priority != next_priority {
            sqlx::query!(
                "UPDATE provider_resource SET priority = $1 where id = $2 returning id",
                i32::from(next_priority),
                id
            )
            .fetch_one(db)
            .await?;
        }
    }
    Ok(())
}

// TODO: this should return an opaque type that indicates the provider is ready to be processed
pub fn filter_scheduled(pending: Vec<PendingProvider>) -> ScheduledProviders {
    ScheduledProviders(
        pending
            .into_iter()
            .filter(|pen| {
                // should always be scraping things that haven't been scraped before
                pen.last_scrape.map_or(true, |last| {
                    let current_time = Utc::now();
                    let scheduled_scrape: DateTime<Utc> = DateTime::from_utc(last, Utc)
                        + Duration::from_std(pen.priority.added_duration()).unwrap();
                    current_time >= scheduled_scrape
                })
            })
            .collect::<Vec<PendingProvider>>(),
    )
}
