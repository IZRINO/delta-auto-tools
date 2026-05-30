use std::sync::Arc;

use reqwest::{cookie::Jar, redirect::Policy, Client};

use crate::delta::{client::headers::browser_headers, error::DeltaError};

#[derive(Debug, Clone, Copy, Default)]
pub struct HttpOptions;

pub fn build_client(_options: HttpOptions) -> Result<(Client, Arc<Jar>), DeltaError> {
    let jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(jar.clone())
        .default_headers(browser_headers())
        .redirect(Policy::none())
        .build()?;
    Ok((client, jar))
}
