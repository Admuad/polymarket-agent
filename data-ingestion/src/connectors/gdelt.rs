use anyhow::Result;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::Deserialize;
use tracing::info;

/// GDELT news connector
/// GDELT provides free global news data for sentiment analysis
pub struct GDELTConnector {
    api_url: String,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct GDELTArticle {
    #[serde(rename = "GKGRECORDID")]
    id: String,
    #[serde(rename = "DocumentIdentifier")]
    url: String,
    #[serde(rename = "SharingImage")]
    image: Option<String>,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Themes")]
    themes: String,
    #[serde(rename = "Locations")]
    locations: String,
    #[serde(rename = "Persons")]
    persons: String,
    #[serde(rename = "Tone")]
    tone: f64,
}

impl GDELTConnector {
    pub fn new() -> Self {
        Self {
            api_url: "https://api.gdeltproject.org/api/v2/doc/doc".to_string(),
            client: Client::new(),
        }
    }

    pub async fn fetch_recent_articles(&self, hours: i64) -> Result<Vec<GDELTArticle>> {
        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(hours);

        // GDELT query format: mode query start end maxrecords format
        let query = format!(
            "{} {} {} {} {} json",
            "artlist",                     // mode
            "*:*",                        // query (all articles)
            start_time.format("%Y%m%d%H%M%S"),
            end_time.format("%Y%m%d%H%M%S"),
            "250"                         // max records
        );

        info!("Fetching articles from GDELT...");

        let response = self.client
            .get(&self.api_url)
            .query(&[("query", &query)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "GDELT API error: {}",
                response.status()
            ));
        }

        let articles: Vec<GDELTArticle> = response.json().await?;

        info!("✅ Fetched {} articles from GDELT", articles.len());

        Ok(articles)
    }

    pub async fn run_continuous(&self) -> Result<()> {
        loop {
            match self.fetch_recent_articles(1).await {
                Ok(articles) => {
                    // TODO: Process articles and send to event bus
                    info!("Processed {} articles", articles.len());
                }
                Err(e) => {
                    eprintln!("Error fetching GDELT articles: {}", e);
                }
            }

            // Wait before next fetch
            tokio::time::sleep(tokio::time::Duration::from_secs(15 * 60)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gdelt_fetch() {
        let connector = GDELTConnector::new();
        let articles = connector.fetch_recent_articles(24).await.unwrap();
        assert!(!articles.is_empty());
        println!("First article: {:?}", articles.first());
    }
}
