use std::time::Duration;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::config::{AppConfig, DetectStrategy, Provider};

static IPV4_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b((?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})\b")
        .expect("valid ipv4 regex")
});

static IPV6_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:[A-Fa-f0-9]{1,4}:){2,7}[A-Fa-f0-9]{1,4}\b").expect("valid ipv6 regex")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResult {
    pub provider_id: String,
    pub provider_name: String,
    pub url: String,
    pub ok: bool,
    pub ip: Option<String>,
    pub raw_excerpt: Option<String>,
    pub status: Option<u16>,
    pub attempts: u32,
    pub elapsed_ms: u128,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionReport {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub strategy: DetectStrategy,
    pub providers: Vec<ProviderResult>,
    pub detected_ips: Vec<String>,
    pub matched: bool,
    pub matched_ip: Option<String>,
    pub allowed_ips: Vec<String>,
}

/// Pull the first IP out of an arbitrary HTTP body. Tries the user-provided
/// regex first, then a strict IPv4 matcher, then IPv6.
fn extract_ip(body: &str, custom: Option<&str>) -> Option<String> {
    if let Some(pat) = custom {
        if !pat.trim().is_empty() {
            if let Ok(re) = Regex::new(pat) {
                if let Some(c) = re.captures(body) {
                    let v = c.get(1).or_else(|| c.get(0)).map(|m| m.as_str().to_string());
                    if let Some(v) = v {
                        let v = v.trim().to_string();
                        if !v.is_empty() {
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    if let Some(m) = IPV4_RE.find(body) {
        return Some(m.as_str().to_string());
    }
    if let Some(m) = IPV6_RE.find(body) {
        return Some(m.as_str().to_string());
    }
    None
}

async fn fetch_one(
    client: reqwest::Client,
    provider: Provider,
    retry: u32,
) -> ProviderResult {
    let started = std::time::Instant::now();
    let mut last_err: Option<String> = None;
    let mut last_status: Option<u16> = None;
    let max_attempts = retry.max(1);
    for attempt in 1..=max_attempts {
        match client.get(&provider.url).send().await {
            Ok(resp) => {
                let status = resp.status();
                last_status = Some(status.as_u16());
                if !status.is_success() {
                    last_err = Some(format!("HTTP {}", status.as_u16()));
                } else {
                    match resp.text().await {
                        Ok(body) => {
                            let trimmed = body.trim();
                            let excerpt: String = trimmed.chars().take(240).collect();
                            let ip = extract_ip(trimmed, provider.extract_regex.as_deref());
                            return ProviderResult {
                                provider_id: provider.id.clone(),
                                provider_name: provider.name.clone(),
                                url: provider.url.clone(),
                                ok: ip.is_some(),
                                ip,
                                raw_excerpt: Some(excerpt),
                                status: last_status,
                                attempts: attempt,
                                elapsed_ms: started.elapsed().as_millis(),
                                error: None,
                            };
                        }
                        Err(e) => last_err = Some(format!("read body: {e}")),
                    }
                }
            }
            Err(e) => last_err = Some(format!("request: {e}")),
        }
        if attempt < max_attempts {
            let backoff = 200u64 * (1u64 << (attempt - 1).min(4));
            sleep(Duration::from_millis(backoff)).await;
        }
    }
    ProviderResult {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        url: provider.url.clone(),
        ok: false,
        ip: None,
        raw_excerpt: None,
        status: last_status,
        attempts: max_attempts,
        elapsed_ms: started.elapsed().as_millis(),
        error: last_err.or_else(|| Some("unknown error".into())),
    }
}

fn client_for(cfg: &AppConfig) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(cfg.request_timeout_ms.min(15_000)))
        .timeout(Duration::from_millis(cfg.request_timeout_ms.min(30_000)))
        .user_agent(concat!("ip-killswitch/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Run detection against the providers in `cfg` (or `override_*` if supplied).
pub async fn run_detection(
    cfg: &AppConfig,
    override_providers: Option<Vec<Provider>>,
    override_allowed: Option<Vec<String>>,
) -> DetectionReport {
    let started_at = Utc::now();
    let providers: Vec<Provider> = override_providers
        .unwrap_or_else(|| cfg.providers.clone())
        .into_iter()
        .filter(|p| p.enabled && !p.url.trim().is_empty())
        .collect();
    let allowed: Vec<String> = override_allowed
        .unwrap_or_else(|| cfg.allowed_ips.clone())
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let strategy = cfg.strategy;

    let client = match client_for(cfg) {
        Ok(c) => c,
        Err(_) => {
            return DetectionReport {
                started_at,
                finished_at: Utc::now(),
                strategy,
                providers: vec![],
                detected_ips: vec![],
                matched: false,
                matched_ip: None,
                allowed_ips: allowed,
            };
        }
    };

    let retry = cfg.retry.max(1);
    let mut handles = Vec::with_capacity(providers.len());
    for p in providers {
        let cli = client.clone();
        handles.push(tokio::spawn(fetch_one(cli, p, retry)));
    }
    let mut results: Vec<ProviderResult> = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(r) = h.await {
            results.push(r);
        }
    }

    let mut detected: Vec<String> = results.iter().filter_map(|r| r.ip.clone()).collect();
    detected.sort();
    detected.dedup();

    let matched_ip = if allowed.is_empty() {
        None
    } else {
        match strategy {
            DetectStrategy::Any => detected
                .iter()
                .find(|ip| allowed.iter().any(|a| a == *ip))
                .cloned(),
            DetectStrategy::All => {
                let ok_count = results.iter().filter(|r| r.ok).count();
                let agree_one = detected.len() == 1
                    && allowed.contains(&detected[0])
                    && ok_count == results.len()
                    && !results.is_empty();
                if agree_one {
                    Some(detected[0].clone())
                } else {
                    None
                }
            }
        }
    };

    DetectionReport {
        started_at,
        finished_at: Utc::now(),
        strategy,
        providers: results,
        detected_ips: detected,
        matched: matched_ip.is_some(),
        matched_ip,
        allowed_ips: allowed,
    }
}
