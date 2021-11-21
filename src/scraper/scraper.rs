use std::time::Instant;

use async_recursion::async_recursion;
use chrono::{NaiveDateTime, Utc};
use futures::StreamExt;
use log::{debug, info, trace};
use crate::models::PendingProvider;

use crate::scraper::{ProviderMedia, ProviderPost, providers::{CredentialRefresh, ProviderErrorHandle}};

use super::{
    ProviderCredentials,
    ProviderResult, providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput}, ScopedProvider,
};

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
    *credential_ref = Some(credentials);
}

#[async_recursion]
async fn request_page<'a>(
    sp: &'a ScopedProvider,
    provider: &dyn Provider,
    state: &ProviderState,
    input: &ScrapeRequestInput,
) -> (InternalScraperStep, Option<ProviderState>) {
    let iteration = state.iteration;
    let error_step = |error| (InternalScraperStep::Error(error), None);
    match provider.unfold(state.to_owned()).await {
        // we have to indicate an error to the consumer and stop iteration on the next cycle
        Err(error) => match &error {
            ProviderFailure::HttpError(http_error) => match provider.on_error(http_error) {
                Ok(ProviderErrorHandle::Halt) => error_step(error),
                Ok(ProviderErrorHandle::RefreshToken(credentials)) => {
                    debug!(
                        "Triggering token refresh flow for {}",
                        provider.id().to_string()
                    );
                    let write_credentials_and_continue = |creds: ProviderCredentials| {
                        write_provider_credentials(provider, creds);
                        request_page(sp, provider, state, input)
                    };
                    match provider.token_refresh(&credentials).await {
                        Ok(CredentialRefresh::Result(credentials)) => {
                            write_credentials_and_continue(credentials).await
                        }
                        Ok(CredentialRefresh::TryLogin) => {
                            debug!("Triggering login flow for {}", provider.id().to_string());
                            match provider.login().await {
                                Ok(credentials) => {
                                    write_credentials_and_continue(credentials).await
                                }
                                Err(err) => {
                                    debug!(
                                        "Error trying to login to {}: {:?}",
                                        provider.id().to_string(),
                                        err
                                    );
                                    error_step(error)
                                }
                            }
                        }
                        Ok(CredentialRefresh::Halt) => error_step(error),
                        _ => error_step(error),
                    }
                }
                _ => error_step(error),
            },
            // TODO: reduce this nested boilerplate by implementing [From] for a result type?
            _ => error_step(error),
        },
        Ok(ProviderStep::End(result)) => (InternalScraperStep::Data(result), None),
        Ok(ProviderStep::NotInitialized) => {
            info!(
                "Skipping {} because the provider was not initialized",
                provider.id().to_string()
            );
            (InternalScraperStep::Exit, None)
        }
        Ok(ProviderStep::Next(result, pagination)) => {
            let page_size = provider.next_page_size(input.last_scrape, iteration);

            let id = sp.destination.clone();
            let maybe_next_url = provider.from_provider_destination(
                &id,
                page_size.clone(),
                Some(pagination.clone()),
            );

            match maybe_next_url {
                Err(err) => error_step(err),
                Ok(url) => {
                    let next_state = ProviderState {
                        id,
                        default_name: input.default_name.clone(),
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
        default_name: input.default_name.clone(),
        url,
        pagination: None,
        iteration: initial_iteration,
    };

    let mut steps = futures::stream::unfold(Some(seed), |maybe_state| async {
        let state = maybe_state?;
        info!("Scraping URL: {:?}", state.url.0);
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
                let total_found_images = page.posts.iter().flat_map(|p| &p.images).collect::<Vec<_>>().len();
                let mut posts: Vec<ProviderPost> = vec![];
                // it SHOULDN'T be possible for us to have seen a post and only
                // have it partially saved... This should be good enough
                for post in page.posts {
                    let has_known_image = post.images.iter().any(|image| input.latest_data.contains(&image.unique_identifier));
                    if has_known_image {
                        break;
                    }
                    posts.push(post)
                }
                let new_image_count = posts.iter().map(|p| &p.images).len();
                info!("Found {} new images in {}", posts.len(), sp);

                scrape_requests.push(ScrapeRequest {
                    date,
                    step: ScraperStep::Data(ProviderResult { posts, ..page }),
                });

                if new_image_count == 0 {
                    info!(
                        "[{}] has finished crawling because it's back to the last scraped data point",
                        sp
                    );
                    break;
                }
                let pagination_limit = provider.max_pagination();
                // only looking at pagination limit if there's at least one image
                // that's been scraped in the past
                if total_found_images > 1 && scrape_requests.len() as u16 > pagination_limit {
                    info!(
                        "[{}] has reached its pagination limit of {}",
                        sp, pagination_limit
                    );
                    break;
                }
                trace!("Waiting for provider rate limit...");
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
