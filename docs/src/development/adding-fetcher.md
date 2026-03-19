# Adding a Fetcher

This guide walks through adding a new data fetcher to `redshank-fetchers`.

## 1. Add the module

Create `redshank-fetchers/src/fetchers/my_source.rs`.

## 2. Implement the struct

```rust,ignore
use crate::{client::FetcherClient, domain::FetcherError};
use serde::{Deserialize, Serialize};

/// Fetches data from My Source.
pub struct MySourceFetcher {
    client: FetcherClient,
}

impl MySourceFetcher {
    /// Creates a new fetcher with the given HTTP client.
    #[must_use]
    pub const fn new(client: FetcherClient) -> Self {
        Self { client }
    }

    /// Fetch records matching `query`.
    ///
    /// # Errors
    ///
    /// Returns [`FetcherError`] on HTTP or parse failure.
    pub async fn fetch(&self, query: &str) -> Result<Vec<MySourceRecord>, FetcherError> {
        // ...
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MySourceRecord {
    pub id: String,
    pub name: String,
}
```

## 3. Write a test first

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_fetch_returns_records() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": "1", "name": "Test Entity"}
            ])))
            .mount(&server)
            .await;

        let client = FetcherClient::with_base_url(server.uri());
        let fetcher = MySourceFetcher::new(client);
        let results = fetcher.fetch("Test Entity").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Test Entity");
    }
}
```

## 4. Register the module

In `redshank-fetchers/src/fetchers/mod.rs`:

```rust,ignore
pub mod my_source;
```

## 5. Add a CLI subcommand

In `redshank-cli/src/main.rs`, add a variant to the `FetchCommand` enum and wire it to `MySourceFetcher::fetch`.

## 6. Document it

Add an entry to [Data Sources Overview](../data-sources/overview.md) and create a page in `docs/src/data-sources/`.
