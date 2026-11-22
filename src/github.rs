//! GitHub access: modulo index (API tree + raw files), release lookup, asset download.
//! Falls back to a cached index when the network is unavailable.

use crate::modulo::{Modulo, Remote};
use crate::settings::Settings;
use crate::util::{self, fail, CodedError, EXIT_GENERAL, EXIT_NETWORK};
use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::blocking::{Client as Http, Response};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::Duration;

pub struct Github {
    http: Http,
    repo: String,
    token: Option<String>,
    api: String,
    raw: String,
}

#[derive(Deserialize, Debug)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Debug)]
pub struct Asset {
    pub name: String,
    /// API URL (works for public and private repos with `Accept: application/octet-stream`).
    pub url: String,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    fetched_at: String,
    manifests: Vec<String>,
}

pub struct Index {
    pub entries: Vec<Remote>,
}

#[derive(Deserialize)]
struct Tree {
    tree: Vec<TreeEntry>,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
}

fn net_err(e: reqwest::Error) -> anyhow::Error {
    let what = if e.is_timeout() {
        "the request timed out"
    } else if e.is_connect() {
        "could not connect to GitHub"
    } else {
        "the request failed"
    };
    fail(
        EXIT_NETWORK,
        format!("network error: {what} ({e})"),
        "check your internet connection; prox uses its cached modulo index when offline",
    )
}

impl Github {
    pub fn new(settings: &Settings) -> Result<Github> {
        let http = Http::builder()
            .user_agent(concat!("prox/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .context("cannot create the HTTP client")?;
        Ok(Github {
            http,
            repo: settings.github_repo.clone(),
            token: settings.token(),
            api: std::env::var("PROX_GITHUB_API")
                .unwrap_or_else(|_| "https://api.github.com".into()),
            raw: std::env::var("PROX_GITHUB_RAW")
                .unwrap_or_else(|_| "https://raw.githubusercontent.com".into()),
        })
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    fn get(&self, url: &str, accept: &str) -> Result<Response> {
        let mut req = self.http.get(url).header("Accept", accept);
        if let Some(t) = &self.token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
        let resp = req.send().map_err(net_err)?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let limited = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0");
        Err(match status {
            StatusCode::UNAUTHORIZED => fail(
                EXIT_GENERAL,
                "GitHub rejected the token (401)",
                "check GITHUB_TOKEN or `prox --settings set github_token <token>`",
            ),
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS if limited => fail(
                EXIT_NETWORK,
                "GitHub API rate limit reached",
                "set a token: `export GITHUB_TOKEN=...` or `prox --settings set github_token <token>`",
            ),
            StatusCode::NOT_FOUND => fail(
                EXIT_GENERAL,
                format!("not found on GitHub: {url}"),
                format!(
                    "check `github_repo` ({}) and that the repo is pushed; private repos need a token",
                    self.repo
                ),
            ),
            s => fail(EXIT_NETWORK, format!("GitHub answered HTTP {s} for {url}"), "try again later"),
        })
    }

    fn raw_url(&self, path: &str) -> String {
        format!("{}/{}/HEAD/{}", self.raw, self.repo, path)
    }

    fn fetch_index(&self) -> Result<Vec<Remote>> {
        let url = format!(
            "{}/repos/{}/git/trees/HEAD?recursive=1",
            self.api, self.repo
        );
        let tree: Tree = self
            .get(&url, "application/vnd.github+json")?
            .json()
            .map_err(net_err)?;
        let mut names: Vec<String> = tree
            .tree
            .iter()
            .filter_map(|e| {
                let rest = e.path.strip_prefix("modulos/")?;
                let (name, file) = rest.split_once('/')?;
                (file == "modulo.json").then(|| name.to_string())
            })
            .collect();
        names.sort();
        let mut out = Vec::new();
        for name in names {
            let raw = self
                .get(
                    &self.raw_url(&format!("modulos/{name}/modulo.json")),
                    "text/plain",
                )?
                .text()
                .map_err(net_err)?;
            match Modulo::parse(&raw) {
                Ok(m) if m.name == name => out.push(Remote { modulo: m, raw }),
                Ok(m) => eprintln!(
                    "{} skipping modulos/{name}: manifest name '{}' does not match its folder",
                    "warning:".yellow().bold(),
                    m.name
                ),
                Err(e) => eprintln!(
                    "{} skipping modulos/{name}: {e:#}",
                    "warning:".yellow().bold()
                ),
            }
        }
        Ok(out)
    }

    /// The remote modulo index; transparently uses the cache when offline.
    pub fn index(&self) -> Result<Index> {
        match self.fetch_index() {
            Ok(entries) => {
                let cache = Cache {
                    fetched_at: chrono::Utc::now().to_rfc3339(),
                    manifests: entries.iter().map(|r| r.raw.clone()).collect(),
                };
                if let Ok(path) = util::index_cache_path() {
                    if let Ok(text) = serde_json::to_vec_pretty(&cache) {
                        let _ = util::write_atomic(&path, &text);
                    }
                }
                Ok(Index { entries })
            }
            Err(e)
                if e.downcast_ref::<CodedError>()
                    .is_some_and(|c| c.code == EXIT_NETWORK) =>
            {
                match load_cache() {
                    Some((entries, when)) => {
                        eprintln!(
                            "{} offline: using the cached modulo index from {when}",
                            "warning:".yellow().bold()
                        );
                        Ok(Index { entries })
                    }
                    None => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Download one template file of a modulo.
    pub fn modulo_file(&self, modulo: &str, rel: &str) -> Result<Vec<u8>> {
        let url = self.raw_url(&format!("modulos/{modulo}/{rel}"));
        let bytes = self
            .get(&url, "application/octet-stream")?
            .bytes()
            .map_err(net_err)?;
        Ok(bytes.to_vec())
    }

    pub fn latest_release(&self) -> Result<Release> {
        let url = format!("{}/repos/{}/releases/latest", self.api, self.repo);
        match self.get(&url, "application/vnd.github+json") {
            Ok(r) => r.json().map_err(net_err),
            Err(e) if e.downcast_ref::<CodedError>().is_some_and(|c| c.code == EXIT_GENERAL && c.msg.starts_with("not found")) => {
                Err(fail(
                    EXIT_GENERAL,
                    format!("{} has no published release yet", self.repo),
                    "create the first GitHub Release (push a tag like v0.1.0) so `prox --update` has something to download",
                ))
            }
            Err(e) => Err(e),
        }
    }

    pub fn download_asset(&self, asset: &Asset) -> Result<Vec<u8>> {
        Ok(self
            .get(&asset.url, "application/octet-stream")?
            .bytes()
            .map_err(net_err)?
            .to_vec())
    }

    /// Cheap reachability probe for `--doctor`.
    pub fn ping(&self) -> Result<String> {
        let url = format!("{}/repos/{}", self.api, self.repo);
        let r = self.get(&url, "application/vnd.github+json")?;
        let remaining = r
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("?")
            .to_string();
        Ok(format!("rate limit remaining: {remaining}"))
    }
}

fn load_cache() -> Option<(Vec<Remote>, String)> {
    let path = util::index_cache_path().ok()?;
    let cache: Cache = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let entries: Vec<Remote> = cache
        .manifests
        .into_iter()
        .filter_map(|raw| {
            Modulo::parse(&raw)
                .ok()
                .map(|modulo| Remote { modulo, raw })
        })
        .collect();
    Some((entries, cache.fetched_at))
}
