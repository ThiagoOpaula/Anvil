//! Modrinth API client — HTTP transport, rate limiting, retries, and
//! streaming downloads with SHA1 verification.

use crate::error::{Error, Result};
use crate::types::{ApiClient, ModVersion, Project};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const BASE_URL: &str = "https://api.modrinth.com/v2";
const USER_AGENT: &str = "anvil/0.1.0 (thiag@github)";
const RATE_LIMIT_INTERVAL: Duration = Duration::from_millis(250);
const MAX_RETRIES: usize = 3;

/// HTTP client for the Modrinth API with rate limiting and retry logic.
pub struct ModrinthApi {
    client: reqwest::Client,
    base_url: String,
    last_request: Mutex<Option<Instant>>,
}

impl ModrinthApi {
    /// Build a `reqwest::Client` with the required User-Agent, 30 s timeout,
    /// and rustls TLS backend.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .use_rustls_tls()
            .build()
            .map_err(Error::Http)?;

        Ok(Self {
            client,
            base_url: BASE_URL.to_string(),
            last_request: Mutex::new(None),
        })
    }

    /// Enforce 250 ms between requests using a monotonic clock.
    async fn rate_limit(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(instant) = *last {
            let elapsed = instant.elapsed();
            if elapsed < RATE_LIMIT_INTERVAL {
                tokio::time::sleep(RATE_LIMIT_INTERVAL - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    /// Internal HTTP helper.
    ///
    /// * Rate-limits before every request.
    /// * On 429: exponential backoff (`2^attempt` seconds), up to `MAX_RETRIES`.
    /// * On network/timeout errors: same backoff, up to `MAX_RETRIES`.
    /// * On any other non-2xx: returns `Error::Api { status, body }`.
    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body_json: Option<&Value>,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);

        for attempt in 0..=MAX_RETRIES {
            self.rate_limit().await;

            let mut req = self.client.request(method.clone(), &url);

            if let Some(body) = body_json {
                req = req.json(body);
            }

            match req.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }

                    // Rate limited — retry with backoff.
                    if response.status().as_u16() == 429 && attempt < MAX_RETRIES {
                        let backoff = 2u64.pow(attempt as u32);
                        tokio::time::sleep(Duration::from_secs(backoff)).await;
                        continue;
                    }

                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    return Err(Error::Api { status, body });
                }
                Err(e) => {
                    // Retry on transient network errors.
                    if attempt < MAX_RETRIES
                        && (e.is_timeout() || e.is_connect() || e.is_request())
                    {
                        let backoff = 2u64.pow(attempt as u32);
                        tokio::time::sleep(Duration::from_secs(backoff)).await;
                        continue;
                    }
                    return Err(Error::Http(e));
                }
            }
        }

        // All retries exhausted on 429.
        Err(Error::Api {
            status: 429,
            body: "too many requests after max retries".to_string(),
        })
    }
}

#[async_trait]
impl ApiClient for ModrinthApi {
    /// `GET /v2/version_file/{hash}?algorithm=sha1`
    ///
    /// Returns `None` when the hash is not found on Modrinth (404).
    async fn get_version_from_hash(&self, sha1: &str) -> Result<Option<ModVersion>> {
        let path = format!("/version_file/{}?algorithm=sha1", sha1);

        match self.request(reqwest::Method::GET, &path, None).await {
            Ok(response) => {
                let version: ModVersion = response.json().await.map_err(Error::Http)?;
                Ok(Some(version))
            }
            Err(Error::Api { status, .. }) if status == 404 => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// `POST /v2/version_file/{hash}/update?algorithm=sha1`
    ///
    /// The body carries `loaders` and `game_versions` so Modrinth returns the
    /// latest version that matches those filters.  Returns `None` on 404.
    async fn get_latest_version(
        &self,
        sha1: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Option<ModVersion>> {
        let path = format!("/version_file/{}/update?algorithm=sha1", sha1);

        let body = serde_json::json!({
            "loaders": loaders,
            "game_versions": game_versions,
        });

        match self.request(reqwest::Method::POST, &path, Some(&body)).await {
            Ok(response) => {
                let version: ModVersion = response.json().await.map_err(Error::Http)?;
                Ok(Some(version))
            }
            Err(Error::Api { status, .. }) if status == 404 => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// `GET /v2/project/{id}`
    async fn get_project(&self, project_id: &str) -> Result<Project> {
        let path = format!("/project/{}", project_id);
        let response = self.request(reqwest::Method::GET, &path, None).await?;
        let project: Project = response.json().await.map_err(Error::Http)?;
        Ok(project)
    }

    /// Streaming download with progress reporting and on-the-fly SHA1 hashing.
    ///
    /// Returns the hex-encoded SHA1 digest of the downloaded file so the
    /// caller can verify integrity against the advertised hash.
    async fn download_file(
        &self,
        url: &str,
        dest: &Path,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Result<String> {
        let response = self.client.get(url).send().await.map_err(Error::Http)?;

        let total = response.content_length();

        // Ensure the destination directory exists.
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        let mut file = std::fs::File::create(dest).map_err(Error::Io)?;
        let mut hasher = Sha1::new();
        let mut downloaded: u64 = 0;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Error::Http)?;
            file.write_all(&chunk).map_err(Error::Io)?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;
            progress(downloaded, total);
        }

        let sha1_hex = hex::encode(hasher.finalize());
        Ok(sha1_hex)
    }
}
