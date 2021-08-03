use super::{
    providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput},
    ProviderCredentials, ProviderResult, ScopedProvider,
};
use crate::scraper::{
    providers::{CredentialRefresh, ProviderErrorHandle},
    ProviderMedia,
};
use async_recursion::async_recursion;
use chrono::{NaiveDateTime, Utc};
use futures::StreamExt;
use log::{debug, info};
use std::time::Instant;

#[derive(Debug)]
pub struct Scrape<'a> {
    pub provider: &'a ScopedProvider,
    pub requests: Vec<ScrapeRequest>,
}

#[derive(Debug)]
pub struct ScrapeRequest {
    pub date: NaiveDateTime,
    pub step: ScraperStep,
}

#[derive(Debug)]
enum InternalScraperStep {
    Data(ProviderResult),
    Error(ProviderFailure),
    Exit,
}

#[derive(Debug)]
pub enum ScraperStep {
    Data(ProviderResult),
    // we only want to forward request related errors to the consumer
    Error(ProviderFailure),
}

fn write_provider_credentials(provider: &dyn Provider, credentials: ProviderCredentials) {
    let creds = provider.credentials();
    let mut credential_ref = creds.write();
    *credential_ref = credentials;
}

#[async_recursion]
async fn request_page<'a>(
    sp: &'a ScopedProvider,
    provider: &dyn Provider,
    state: &ProviderState,
    input: &ScrapeRequestInput,
) -> (InternalScraperStep, Option<ProviderState>) {
    let iteration = state.iteration;
    match provider.unfold(state.to_owned()).await {
        // we have to indicate an error to the consumer and stop iteration on the next cycle
        Err(error) => match &error {
            ProviderFailure::HttpError(http_error) => match provider.on_error(http_error) {
                Ok(ProviderErrorHandle::Halt) => (InternalScraperStep::Error(error), None),
                Ok(ProviderErrorHandle::RefreshToken(credentials)) => {
                    debug!(
                        "Triggering token refresh flow for {}",
                        provider.id().to_string()
                    );
                    match provider.token_refresh(&credentials).await {
                        Ok(CredentialRefresh::Result(credentials)) => {
                            write_provider_credentials(provider, credentials);
                            request_page(sp, provider, state, input).await
                        }
                        Ok(CredentialRefresh::TryLogin) => {
                            debug!("Triggering login flow for {}", provider.id().to_string());
                            match provider.login().await {
                                Ok(credentials) => {
                                    write_provider_credentials(provider, credentials);
                                    request_page(sp, provider, state, input).await
                                }
                                Err(_error) => (InternalScraperStep::Error(error), None),
                            }
                        }
                        Ok(CredentialRefresh::Halt) => (InternalScraperStep::Error(error), None),
                        _ => (InternalScraperStep::Error(error), None),
                    }
                }
                _ => (InternalScraperStep::Error(error), None),
            },
            // TODO: reduce this nested boilerplate by implementing [From] for a result type?
            _ => (InternalScraperStep::Error(error), None),
        },
        Ok(ProviderStep::End(result)) => (InternalScraperStep::Data(result), None),
        Ok(ProviderStep::NotInitialized) => (InternalScraperStep::Exit, None),
        Ok(ProviderStep::Next(result, pagination)) => {
            let page_size = provider.next_page_size(input.last_scrape, iteration);

            let id = sp.destination.clone();
            let maybe_next_url = provider.from_provider_destination(
                &id,
                page_size.clone(),
                Some(pagination.clone()),
            );

            match maybe_next_url {
                Err(err) => (InternalScraperStep::Error(err), None),
                Ok(url) => {
                    let next_state = ProviderState {
                        id,
                        url: url.clone(),
                        pagination: Some(pagination.clone()),
                        iteration: iteration + 1,
                    };
                    (InternalScraperStep::Data(result), Some(next_state))
                }
            }
        }
    }
}

pub async fn scrape<'a>(
    sp: &'a ScopedProvider,
    provider: &dyn Provider,
    input: &ScrapeRequestInput,
) -> Result<Scrape<'a>, ProviderFailure> {
    let initial_iteration = 0;
    let page_size = provider.next_page_size(input.last_scrape, initial_iteration);
    let id = sp.destination.clone();
    let url = provider.from_provider_destination(&id, page_size.to_owned(), None)?;

    let seed = ProviderState {
        id: id.clone(),
        url,
        pagination: None,
        iteration: initial_iteration,
    };

    let mut steps = futures::stream::unfold(Some(seed), |maybe_state| async {
        let state = maybe_state?;
        debug!("Scraping URL: {:?}", state.url.0);
        Some(request_page(sp, provider, &state, input).await)
    })
    .boxed_local();

    let mut scrape_requests: Vec<ScrapeRequest> = vec![];
    let scrape_start = Instant::now();

    while let Some(step) = steps.next().await {
        let date = Utc::now().naive_utc();
        match step {
            InternalScraperStep::Exit => break,
            InternalScraperStep::Error(error) => {
                scrape_requests.push(ScrapeRequest {
                    date,
                    step: ScraperStep::Error(error),
                });
                // no reason to continue scraping after an error
                break;
            }
            InternalScraperStep::Data(page) => {
                let original_image_count = page.images.len();
                let images = page
                    .images
                    // TODO: remove this clone using Rc?
                    .clone()
                    .into_iter()
                    .take_while(|r| !input.latest_data.contains(&r.unique_identifier))
                    .collect::<Vec<ProviderMedia>>();
                let new_image_count = images.len();
                debug!("Found {} new images in {}", images.len(), sp);

                scrape_requests.push(ScrapeRequest {
                    date,
                    step: ScraperStep::Data(ProviderResult { images, ..page }),
                });

                let has_known_image = new_image_count != original_image_count;
                if has_known_image {
                    info!(
                        "[{}] has finished crawling because it's back to the last scraped data point",
                        sp
                    );
                    break;
                }
                let pagination_limit = provider.max_pagination();
                if scrape_requests.len() as u16 > pagination_limit {
                    info!(
                        "[{}] has reached its pagination limit of {}",
                        sp, pagination_limit
                    );
                    break;
                }
                provider.wait(&sp.destination).await;
            }
        }
    }
    let scrape_count = scrape_requests.len();
    info!(
        "[{}] finished scraping in {:?} after {} request{}",
        sp,
        scrape_start.elapsed(),
        scrape_count,
        if scrape_count != 1 { "s" } else { "" }
    );
    Ok(Scrape {
        provider: &sp,
        requests: scrape_requests,
    })
}
