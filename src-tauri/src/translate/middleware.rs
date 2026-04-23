use super::engine::*;
use super::plugin::{HealthStatus, PluginConfig, PluginMetadata, TranslationPlugin};
use crate::error::TranslateError;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, Instant};

pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
        }
    }
}

pub struct RateLimitPolicy {
    pub requests_per_second: f32,
    pub burst_size: u32,
}

impl Default for RateLimitPolicy {
    fn default() -> Self {
        Self {
            requests_per_second: 5.0,
            burst_size: 10,
        }
    }
}

struct CacheEntry {
    result: String,
    inserted_at: Instant,
}

pub struct TranslationCache {
    entries: HashMap<String, CacheEntry>,
    ttl: Duration,
    max_entries: usize,
}

impl TranslationCache {
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
            max_entries,
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).and_then(|entry| {
            if entry.inserted_at.elapsed() < self.ttl {
                Some(entry.result.as_str())
            } else {
                None
            }
        })
    }

    pub fn insert(&mut self, key: String, result: String) {
        if self.entries.len() >= self.max_entries {
            let oldest_key = self
                .entries
                .iter()
                .min_by_key(|(_, v)| v.inserted_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest_key {
                self.entries.remove(&k);
            }
        }
        self.entries.insert(key, CacheEntry {
            result,
            inserted_at: Instant::now(),
        });
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn cache_key(text: &str, source_lang: &str, target_lang: &str, namespace: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    source_lang.hash(&mut hasher);
    target_lang.hash(&mut hasher);
    namespace.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub struct MiddlewarePlugin {
    inner: Box<dyn TranslationPlugin>,
    retry_policy: RetryPolicy,
    rate_limiter: Arc<RwLock<RateLimiterState>>,
    cache: Arc<RwLock<TranslationCache>>,
}

struct RateLimiterState {
    tokens: f32,
    last_refill: Instant,
    refill_rate: f32,
    burst_size: u32,
}

impl RateLimiterState {
    fn new(policy: &RateLimitPolicy) -> Self {
        Self {
            tokens: policy.burst_size as f32,
            last_refill: Instant::now(),
            refill_rate: policy.requests_per_second,
            burst_size: policy.burst_size,
        }
    }

    async fn acquire(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f32();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.burst_size as f32);
        self.last_refill = now;

        if self.tokens < 1.0 {
            let wait = (1.0 - self.tokens) / self.refill_rate;
            tokio::time::sleep(Duration::from_secs_f32(wait)).await;
            self.tokens = 0.0;
            self.last_refill = Instant::now();
        } else {
            self.tokens -= 1.0;
        }
    }
}

impl MiddlewarePlugin {
    pub fn wrap(
        plugin: Box<dyn TranslationPlugin>,
        retry_policy: RetryPolicy,
        rate_limit_policy: RateLimitPolicy,
    ) -> Self {
        Self {
            inner: plugin,
            retry_policy,
            rate_limiter: Arc::new(RwLock::new(RateLimiterState::new(&rate_limit_policy))),
            cache: Arc::new(RwLock::new(TranslationCache::new(
                Duration::from_secs(3600),
                10000,
            ))),
        }
    }

    pub fn wrap_default(plugin: Box<dyn TranslationPlugin>) -> Self {
        Self::wrap(plugin, RetryPolicy::default(), RateLimitPolicy::default())
    }
}

#[async_trait]
impl TranslationPlugin for MiddlewarePlugin {
    fn metadata(&self) -> &PluginMetadata {
        self.inner.metadata()
    }

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &PluginConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let ns = &self.inner.metadata().namespace;
        let total = request.texts.len();
        let mut results = Vec::with_capacity(total);
        let mut cached_count = 0;

        {
            let cache = self.cache.read().await;
            for text in &request.texts {
                let key = cache_key(text, &request.source_lang, &request.target_lang, ns);
                if let Some(cached) = cache.get(&key) {
                    results.push(cached.to_string());
                    cached_count += 1;
                } else {
                    results.push(String::new());
                }
            }
        }

        let uncached_indices: Vec<usize> = results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_empty())
            .map(|(i, _)| i)
            .collect();

        if uncached_indices.is_empty() {
            let _ = progress_tx.send(TranslateProgress {
                percent: 100.0,
                translated_count: total,
                total_count: total,
            }).await;
            return Ok(TranslateResult {
                texts: results,
                engine: ns.clone(),
            });
        }

        let uncached_texts: Vec<String> = uncached_indices.iter().map(|&i| request.texts[i].clone()).collect();
        let uncached_request = TranslateRequest {
            texts: uncached_texts,
            source_lang: request.source_lang.clone(),
            target_lang: request.target_lang.clone(),
            context_hint: request.context_hint.clone(),
        };

        let mut attempt = 0;

        loop {
            {
                let mut limiter = self.rate_limiter.write().await;
                limiter.acquire().await;
            }

            match self.inner.translate(&uncached_request, config, progress_tx.clone()).await {
                Ok(translate_result) => {
                    let mut cache = self.cache.write().await;
                    for (i, translated) in translate_result.texts.iter().enumerate() {
                        let original_idx = uncached_indices[i];
                        results[original_idx] = translated.clone();
                        let key = cache_key(
                            &request.texts[original_idx],
                            &request.source_lang,
                            &request.target_lang,
                            ns,
                        );
                        cache.insert(key, translated.clone());
                    }
                    tracing::info!(
                        "Translation complete: {} total, {} from cache, {} fresh ({})",
                        total, cached_count, uncached_indices.len(), ns
                    );
                    return Ok(TranslateResult {
                        texts: results,
                        engine: translate_result.engine,
                    });
                }
                Err(e) => {
                    let is_retryable = match &e {
                        TranslateError::Network(_) => true,
                        TranslateError::RateLimited { .. } => true,
                        TranslateError::Api { status, .. } => *status >= 500 || *status == 429,
                        _ => false,
                    };

                    if !is_retryable || attempt >= self.retry_policy.max_retries {
                        return Err(e);
                    }

                    attempt += 1;
                    let delay = self.retry_policy.base_delay * 2u32.saturating_pow(attempt - 1);
                    let delay = delay.min(self.retry_policy.max_delay);
                    tracing::warn!(
                        "Translation attempt {}/{} failed ({}), retrying in {:?}...",
                        attempt,
                        self.retry_policy.max_retries,
                        e,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn health_check(&self, config: &PluginConfig) -> HealthStatus {
        self.inner.health_check(config).await
    }
}
