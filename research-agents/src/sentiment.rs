//! Sentiment Agent - Analyzes news sentiment for market impact
//!
//! This agent processes news data from Layer 0 (GDELT, etc.) and:
//! - Calculates sentiment scores using simple NLP
//! - Matches news themes to market categories
//! - Generates sentiment signals with confidence scores
//!
//! Future enhancements:
//! - Use rust-bert for advanced sentiment analysis
//! - Topic modeling for better theme matching
//! - Temporal sentiment tracking

use super::agent::{Agent, AgentConfig, AgentInput, AgentOutput, AgentStatus, ControlMessage, ControlResponse};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;
use common::Market;

/// Sentiment score with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentScore {
    pub score: f64,        // -1.0 (very negative) to 1.0 (very positive)
    pub magnitude: f64,    // 0.0 (neutral) to 1.0 (strong)
    pub confidence: f64,   // 0.0 to 1.0
    pub article_count: u32,
}

/// Sentiment signal output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentSignal {
    pub market_id: Uuid,
    pub market_category: String,
    pub sentiment: SentimentScore,
    pub top_themes: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub sources: Vec<String>,
}

/// News article from GDELT (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsArticle {
    pub id: String,
    pub url: String,
    pub title: String,
    pub themes: String,
    pub tone: f64,
    pub timestamp: DateTime<Utc>,
}

/// Sentiment agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAgentConfig {
    pub base: AgentConfig,
    pub min_articles_threshold: u32,
    pub sentiment_threshold: f64,
    pub theme_weight: f64,
    pub tone_weight: f64,
    pub category_keywords: HashMap<String, Vec<String>>,
}

impl Default for SentimentAgentConfig {
    fn default() -> Self {
        let mut category_keywords: HashMap<String, Vec<String>> = HashMap::new();

        // Polymarket common categories
        category_keywords.insert("Politics".to_string(), vec![
            "election".to_string(), "vote".to_string(), "president".to_string(), "congress".to_string(), "senate".to_string(), "campaign".to_string(),
            "republican".to_string(), "democrat".to_string(), "policy".to_string(), "government".to_string(), "legislation".to_string(),
            "trump".to_string(), "biden".to_string(), "white house".to_string(), "capitol".to_string(),
        ]);

        category_keywords.insert("Economics".to_string(), vec![
            "inflation".to_string(), "gdp".to_string(), "economy".to_string(), "recession".to_string(), "unemployment".to_string(),
            "interest rate".to_string(), "federal reserve".to_string(), "fed".to_string(), "stock market".to_string(),
            "crypto".to_string(), "bitcoin".to_string(), "ethereum".to_string(), "finance".to_string(),
        ]);

        category_keywords.insert("Geopolitics".to_string(), vec![
            "war".to_string(), "conflict".to_string(), "invasion".to_string(), "military".to_string(), "sanctions".to_string(),
            "diplomacy".to_string(), "treaty".to_string(), "nuclear".to_string(), "russia".to_string(), "ukraine".to_string(),
            "china".to_string(), "israel".to_string(), "palestine".to_string(), "iran".to_string(),
        ]);

        category_keywords.insert("Technology".to_string(), vec![
            "ai".to_string(), "artificial intelligence".to_string(), "tech".to_string(), "software".to_string(), "startup".to_string(),
            "innovation".to_string(), "cybersecurity".to_string(), "data".to_string(), "cloud".to_string(), "platform".to_string(),
            "regulation".to_string(), "antitrust".to_string(), "monopoly".to_string(),
        ]);

        category_keywords.insert("Climate".to_string(), vec![
            "climate".to_string(), "warming".to_string(), "carbon".to_string(), "emissions".to_string(), "renewable".to_string(),
            "energy".to_string(), "solar".to_string(), "wind".to_string(), "weather".to_string(), "disaster".to_string(),
            "cop".to_string(), "paris".to_string(), "agreement".to_string(), "green".to_string(),
        ]);

        Self {
            base: AgentConfig {
                agent_id: "sentiment-agent".to_string(),
                name: "Sentiment Analysis Agent".to_string(),
                enabled: true,
                max_markets_per_batch: 50,
                processing_interval_secs: 300, // 5 minutes
            },
            min_articles_threshold: 3,
            sentiment_threshold: 0.2,
            theme_weight: 0.6,
            tone_weight: 0.4,
            category_keywords,
        }
    }
}

/// Sentiment Agent - analyzes news for market sentiment
pub struct SentimentAgent {
    config: SentimentAgentConfig,
    status: Arc<RwLock<AgentStatus>>,
    articles: Arc<RwLock<Vec<NewsArticle>>>,
    sentiment_cache: Arc<RwLock<HashMap<Uuid, SentimentScore>>>,
    start_time: std::time::Instant,
}

impl SentimentAgent {
    /// Create a new sentiment agent
    pub fn new(config: SentimentAgentConfig) -> Self {
        Self {
            config,
            status: Arc::new(RwLock::new(AgentStatus::Idle)),
            articles: Arc::new(RwLock::new(Vec::new())),
            sentiment_cache: Arc::new(RwLock::new(HashMap::new())),
            start_time: std::time::Instant::now(),
        }
    }

    /// Add news articles for processing
    pub async fn add_articles(&self, articles: Vec<NewsArticle>) {
        let count = articles.len();
        let mut store = self.articles.write().await;
        store.extend(articles);
        debug!("Added {} articles, total: {}", count, store.len());
    }

    /// Clear cached articles (call after processing)
    pub async fn clear_articles(&self) {
        self.articles.write().await.clear();
    }

    /// Calculate sentiment from articles using simple NLP
    fn calculate_sentiment(&self, articles: &[NewsArticle]) -> SentimentScore {
        if articles.is_empty() {
            return SentimentScore {
                score: 0.0,
                magnitude: 0.0,
                confidence: 0.0,
                article_count: 0,
            };
        }

        let article_count = articles.len() as u32;

        // Method 1: Use GDELT's tone score (-100 to +100)
        let tone_sum: f64 = articles.iter()
            .map(|a| a.tone)
            .sum();

        let avg_tone = tone_sum / articles.len() as f64;
        let tone_sentiment = (avg_tone / 100.0).clamp(-1.0, 1.0);

        // Method 2: Simple keyword-based sentiment (very basic)
        let keyword_sentiment = self.calculate_keyword_sentiment(articles);

        // Combine methods
        let score = (tone_sentiment * self.config.tone_weight) +
                   (keyword_sentiment * self.config.theme_weight);

        // Magnitude = strength of sentiment (absolute value)
        let magnitude = score.abs();

        // Confidence based on article count and consistency
        let confidence = if article_count >= self.config.min_articles_threshold {
            let count_factor = (article_count as f64 / self.config.min_articles_threshold as f64)
                .min(2.0) / 2.0;
            let consistency_factor = 1.0 - (articles.iter()
                .map(|a| a.tone)
                .collect::<Vec<_>>()
                .windows(2)
                .map(|w| (w[0] - w[1]).abs() / 100.0)
                .sum::<f64>() / articles.len().saturating_sub(1).max(1) as f64);
            (count_factor + consistency_factor) / 2.0
        } else {
            (article_count as f64 / self.config.min_articles_threshold as f64).max(0.1)
        };

        SentimentScore {
            score: score.clamp(-1.0, 1.0),
            magnitude: magnitude.clamp(0.0, 1.0),
            confidence: confidence.clamp(0.0, 1.0),
            article_count,
        }
    }

    /// Simple keyword-based sentiment analysis
    fn calculate_keyword_sentiment(&self, articles: &[NewsArticle]) -> f64 {
        // Very basic sentiment lexicon (negative/positive words)
        let negative_words = vec![
            "crisis", "crash", "drop", "fall", "decline", "decrease",
            "loss", "fail", "bad", "negative", "worst", "downward",
            "bearish", "sell", "dump", "collapse", "risk", "danger",
            "threat", "attack", "war", "conflict", "inflation", "recession",
        ];

        let positive_words = vec![
            "growth", "rise", "increase", "gain", "profit", "success",
            "good", "positive", "best", "upward", "bullish", "buy",
            "recovery", "boom", "breakthrough", "win", "victory",
            "peace", "agreement", "deal", "lower", "cut", "reduce",
        ];

        let mut total_score = 0.0;
        let mut total_words = 0.0;

        for article in articles {
            let text = format!("{} {}", article.title, article.themes).to_lowercase();

            for word in &negative_words {
                if text.contains(word) {
                    total_score -= 1.0;
                    total_words += 1.0;
                }
            }

            for word in &positive_words {
                if text.contains(word) {
                    total_score += 1.0;
                    total_words += 1.0;
                }
            }
        }

        if total_words > 0.0 {
            let result: f64 = total_score / total_words;
            result.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    /// Extract themes from articles
    fn extract_themes(&self, articles: &[NewsArticle], limit: usize) -> Vec<String> {
        let mut theme_counts: HashMap<String, u32> = HashMap::new();

        for article in articles {
            for theme in article.themes.split(';') {
                let theme = theme.trim().to_lowercase();
                if !theme.is_empty() {
                    *theme_counts.entry(theme).or_insert(0) += 1;
                }
            }
        }

        let mut themes: Vec<(String, u32)> = theme_counts.into_iter().collect();
        themes.sort_by(|a, b| b.1.cmp(&a.1));

        themes.into_iter()
            .take(limit)
            .map(|(t, _)| t)
            .collect()
    }

    /// Match articles to market category
    fn match_to_category(&self, market: &Market) -> Vec<NewsArticle> {
        let _category = market.category.to_lowercase();
        let question = market.question.to_lowercase();
        let description = market.description.to_lowercase();

        let keywords = self.config.category_keywords
            .get(&market.category)
            .cloned()
            .unwrap_or_default();

        let articles = self.articles.try_read();

        if let Ok(articles) = articles {
            articles.iter()
                .filter(|article| {
                    let article_text = format!("{} {}", article.title, article.themes).to_lowercase();

                    // Check category match
                    let category_match = if let Some(cats) = self.config.category_keywords.get(&market.category) {
                        cats.iter().any(|kw| article_text.contains(kw))
                    } else {
                        false
                    };

                    // Check keyword match in question/description
                    let keyword_match = keywords.iter()
                        .any(|kw| question.contains(kw) || description.contains(kw));

                    // Check for theme overlap
                    let theme_overlap = article.themes.to_lowercase()
                        .split(';')
                        .any(|t| keywords.contains(&t.trim().to_lowercase()));

                    category_match || keyword_match || theme_overlap
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[async_trait]
impl Agent for SentimentAgent {
    fn config(&self) -> &AgentConfig {
        &self.config.base
    }

    fn status(&self) -> AgentStatus {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                *self.status.read().await
            })
        })
    }

    async fn process_market(&self, input: AgentInput) -> Result<Option<AgentOutput>> {
        let market = input.market;
        let start = std::time::Instant::now();

        // Update status
        *self.status.write().await = AgentStatus::Processing;

        // Match articles to this market
        let relevant_articles = self.match_to_category(&market);

        if relevant_articles.is_empty() {
            debug!("No relevant articles for market {}", market.id);
            *self.status.write().await = AgentStatus::Idle;
            return Ok(None);
        }

        // Calculate sentiment
        let sentiment = self.calculate_sentiment(&relevant_articles);

        // Check threshold
        if sentiment.magnitude < self.config.sentiment_threshold {
            debug!("Sentiment magnitude {} below threshold {} for market {}",
                   sentiment.magnitude, self.config.sentiment_threshold, market.id);
            *self.status.write().await = AgentStatus::Idle;
            return Ok(None);
        }

        // Extract themes
        let top_themes = self.extract_themes(&relevant_articles, 5);

        // Build signal
        let signal = SentimentSignal {
            market_id: market.id,
            market_category: market.category.clone(),
            sentiment: sentiment.clone(),
            top_themes,
            timestamp: Utc::now(),
            sources: relevant_articles.iter()
                .take(10)
                .map(|a| a.id.clone())
                .collect(),
        };

        // Cache sentiment
        self.sentiment_cache.write().await.insert(market.id, sentiment.clone());

        *self.status.write().await = AgentStatus::Idle;

        Ok(Some(AgentOutput {
            agent_id: self.config.base.agent_id.clone(),
            market_id: market.id,
            signal_type: "sentiment".to_string(),
            data: serde_json::to_value(signal)?,
            confidence: sentiment.confidence,
            timestamp: Utc::now(),
            processing_time_ms: start.elapsed().as_millis() as u64,
        }))
    }

    async fn handle_control(&self, msg: ControlMessage) -> Result<ControlResponse> {
        match msg {
            ControlMessage::Pause => {
                *self.status.write().await = AgentStatus::Paused;
                Ok(ControlResponse::Ok)
            }
            ControlMessage::Resume => {
                *self.status.write().await = AgentStatus::Idle;
                Ok(ControlResponse::Ok)
            }
            ControlMessage::HealthCheck => {
                let uptime = self.start_time.elapsed().as_secs();
                Ok(ControlResponse::HealthCheck {
                    status: *self.status.read().await,
                    uptime_secs: uptime,
                })
            }
            ControlMessage::Shutdown => {
                *self.status.write().await = AgentStatus::Idle;
                Ok(ControlResponse::Ok)
            }
            ControlMessage::UpdateConfig(_config) => {
                warn!("Sentiment agent config updates not implemented yet");
                Ok(ControlResponse::Ok)
            }
        }
    }

    async fn on_start(&self) -> Result<()> {
        info!("Sentiment agent starting");
        *self.status.write().await = AgentStatus::Idle;
        Ok(())
    }

    async fn on_stop(&self) -> Result<()> {
        info!("Sentiment agent stopping");
        *self.status.write().await = AgentStatus::Idle;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sentiment_agent_creation() {
        let config = SentimentAgentConfig::default();
        let agent = SentimentAgent::new(config);
        assert_eq!(agent.config().agent_id, "sentiment-agent");
    }

    #[test]
    fn test_calculate_sentiment_empty() {
        let config = SentimentAgentConfig::default();
        let agent = SentimentAgent::new(config);
        let sentiment = agent.calculate_sentiment(&[]);
        assert_eq!(sentiment.article_count, 0);
        assert_eq!(sentiment.confidence, 0.0);
    }

    #[test]
    fn test_extract_themes() {
        let config = SentimentAgentConfig::default();
        let agent = SentimentAgent::new(config);

        let articles = vec![
            NewsArticle {
                id: "1".to_string(),
                url: "https://example.com".to_string(),
                title: "Election news".to_string(),
                themes: "ELECTION;POLITICS;USA".to_string(),
                tone: 50.0,
                timestamp: Utc::now(),
            },
        ];

        let themes = agent.extract_themes(&articles, 5);
        assert!(!themes.is_empty());
    }
}
