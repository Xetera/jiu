use crate::{
    db::Database,
    models::PendingProvider,
    scraper::{AllProviders, ScopedProvider},
};
use parking_lot::RwLock;
use std::{collections::HashSet, str::FromStr};

pub type RunningProviders = HashSet<ScopedProvider>;

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
        WHERE pr.enabled"
    )
    .fetch_all(db)
    .await?;
    // let provider_results = providers
    //     .into_iter()
    //     .into_group_map_by(|rec| (rec.name, rec.destination));

    let running = running_providers.read();
    Ok(potential_target_providers
        .into_iter()
        .map(|row| PendingProvider {
            id: row.id,
            provider: ScopedProvider {
                destination: row.destination,
                name: AllProviders::from_str(&row.name).unwrap(),
            },
            last_scrape: row.last_scrape,
        })
        .filter(|sp| !running.contains(&sp.provider))
        .collect::<Vec<_>>())
    // let mut out: Vec<ScrapeHistory> = vec![];
    // for ((name, destination), rows) in provider_results {
    //     let a = rows.iter().group_by(|a| a.ed).collect();
    //     let rows_ = rows.iter().map(|row| ScrapeHistory {
    //         date: row.last_scrape,
    //         priority: Priority::unchecked_clamp(row.priority.unwrap().try_into().unwrap()),
    //         result_count: row,
    //         provider: ScopedProvider {
    //             destination,
    //             name: AllProviders::from_str(&name).unwrap(),
    //         },
    //     });
    //     out.extend(rows_);
    //     // .map(|p| )
    //     // value.iter().map(|r| r)
    // }
    // acc.push(ScrapeHistory {
    //     date: b.date,
    //     priority: b.priority,
    //     provider: ,
    // });
    // acc
    // let results = sqlx::query!("SELECT * FROM scrape WHERE id = ANY($1)", &provider_ids[..])
    //     .fetch_all(db)
    //     .await?;
}

pub async fn mark_as_scheduled(
    db: &Database,
    pending_providers: &[PendingProvider],
    running_providers: &RwLock<RunningProviders>,
) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE provider_resource SET last_queue = NOW() WHERE id = ANY($1)",
        &pending_providers.iter().map(|pv| pv.id).collect::<Vec<_>>(),
        // &provider_ids[..]
    )
    .fetch_optional(db)
    .await?;
    let mut handle = running_providers.write();
    handle.extend(pending_providers.iter().map(|pp| pp.provider.clone()));
    Ok(())
}
