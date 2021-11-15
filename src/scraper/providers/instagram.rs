// use super::*;
// use num_traits::identities;
// use reqwest::Client;
// use std::sync::Arc;
// 
// use crate::scheduler::UnscopedLimiter;
// 
// pub struct InstagramProfile {
//     pub client: Arc<Client>,
//     pub rate_limiter: UnscopedLimiter,
// }
// 
// impl Provider for InstagramProfile {
//     fn id(&self) -> AllProviders {
//         AllProviders::InstagramProfile
//     }
//     fn new(input: ProviderInput) -> Self
//         where
//             Self: Sized,
//     {
//         Self {
//             credentials: create_credentials(),
//             client: Arc::clone(&input.client),
//             rate_limiter: Self::rate_limiter(),
//         }
//     }
//     // fn
// }
