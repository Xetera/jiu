use std::time::Instant;

use async_recursion::async_recursion;
use chrono::{NaiveDateTime, Utc};
use futures::StreamExt;
use log::{debug, error, info, trace};
use crate::api::v1::ProviderStat;

use crate::scraper::{
    providers::{CredentialRefresh, ProviderErrorHandle},
    ProviderPost,
};
use crate::scraper::scraper::ScraperErrorHandleDecision::{Continue, MaxLoginAttempts};

use super::{
    providers::{Provider, ProviderFailure, ProviderState, ProviderStep, ScrapeRequestInput},
    ProviderCredentials, ProviderResult, ScopedProvider,
};

#[derive(Debug)]
pub struct Scrape<'a> {
    pub provider: &'a ScopedProvider,
    pub requests: Vec<ScrapeRequest>,
}

impl Scrape<'_> {
    pub fn discovered_new_images(&self) -> bool {
        let step = match self.requests.get(0) {
            None => return false,
            Some(req) => &req.step,
        };
        match step {
            ScraperStep::Data(data) => !data.posts.is_empty(),
            ScraperStep::Error(_) => false,
        }
    }
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

enum ScraperErrorHandleDecision {
    Continue,
    MaxLoginAttempts(u32),
}

fn write_provider_credentials(provider: &dyn Provider, credentials: ProviderCredentials) {
    let creds = provider.credentials();
    let mut credential_ref = creds.write();
    *credential_ref = Some(credentials);
}

fn should_continue_requests(state: &ProviderState, provider: &dyn Provider) -> ScraperErrorHandleDecision {
    let max_attempts = provider.max_login_attempts();
    if state.login_attempts > max_attempts {
        error!("Failed to login to {} after {} attempts. Giving up.", provider.id().to_string(), max_attempts);
        return MaxLoginAttempts(max_attempts);
    }
    return Continue;
}

#[async_recursion]
async fn request_page<'a>(
    sp: &'a ScopedProvider,
    provider: &dyn Provider,
    state: ProviderState,
    input: &ScrapeRequestInput,
) -> (InternalScraperStep, Option<ProviderState>) {
    let iteration = state.iteration;
    let error_step = |error| {
        debug!("Exiting scrape due to an error {:?}", error);
        (InternalScraperStep::Error(error), None)
    };
    let give_up = (InternalScraperStep::Exit, None);
    let write_credentials_and_continue = |creds: ProviderCredentials| {
        write_provider_credentials(provider, creds);
        let new_state = ProviderState {
            login_attempts: state.login_attempts + 1,
            ..state.clone()
        };
        request_page(sp, provider, new_state, input)
    };
    match provider.unfold(state.to_owned()).await {
        // we have to indicate an error to the consumer and stop iteration on the next cycle
        Err(error) => match &error {
            ProviderFailure::HttpError(http_error) => match provider.on_error(http_error) {
                Ok(ProviderErrorHandle::Halt) => error_step(error),
                Ok(ProviderErrorHandle::Login) => {
                    if let MaxLoginAttempts(count) = should_continue_requests(&state, provider) {
                        error!("Too many login attempts ({}) for {}. Giving up", count, provider.id().to_string());
                        return give_up;
                    }
                    debug!("Triggering login flow for {}", provider.id().to_string());
                    match provider.login().await {
                        Ok(credentials) => write_credentials_and_continue(credentials).await,
                        Err(error) => error_step(error),
                    }
                }
                Ok(ProviderErrorHandle::RefreshToken(credentials)) => {
                    if let MaxLoginAttempts(count) = should_continue_requests(&state, provider) {
                        error!("Too many login attempts ({}) for {}. Giving up", count, provider.id().to_string());
                        return give_up;
                    }
                    debug!(
                        "Triggering token refresh flow for {}",
                        provider.id().to_string()
                    );
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
            let page_size = if input.is_first_scrape {
                provider.max_page_size()
            } else {
                provider.default_page_size()
            };

            let id = sp.destination.clone();
            let maybe_next_url =
                provider.from_provider_destination(&id, page_size, Some(pagination.clone()));

            match maybe_next_url {
                Err(err) => error_step(err),
                Ok(url) => {
                    let next_state = ProviderState {
                        id,
                        default_name: input.default_name.clone(),
                        url,
                        pagination: Some(pagination),
                        iteration: iteration + 1,
                        ..state.clone()
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
    let page_size = if input.is_first_scrape {
        provider.max_page_size()
    } else {
        provider.default_page_size()
    };
    let id = sp.destination.clone();
    let url = provider.from_provider_destination(&id, page_size.to_owned(), None)?;

    let seed = ProviderState {
        login_attempts: 0,
        id: id.clone(),
        default_name: input.default_name.clone(),
        url,
        pagination: None,
        iteration: initial_iteration,
    };

    let mut steps = futures::stream::unfold(Some(seed), |maybe_state| async {
        let state = maybe_state?;
        info!("Scraping URL: {:?}", state.url.0);
        Some(request_page(sp, provider, state, input).await)
    })
        .boxed();

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
                let total_found_images = page.posts.iter().flat_map(|p| &p.images).count();
                let mut posts: Vec<ProviderPost> = vec![];
                for post in page.posts {
                    // it SHOULDN'T be possible for us to have seen a post and only
                    // have it partially saved... This should be good enough
                    // This does sadly break the debugging process if you're deleting images
                    // from the db and re-scraping to trigger things
                    let known_image = post
                        .images
                        .iter()
                        .find(|image| input.latest_data.contains(&image.unique_identifier));
                    if let Some(image) = known_image {
                        debug!(
                            "Reached last known image id for {}: {}",
                            sp, image.unique_identifier
                        );
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
                if !input.latest_data.is_empty() && scrape_requests.len() as u16 > pagination_limit
                {
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
        provider: sp,
        requests: scrape_requests,
    })
}
