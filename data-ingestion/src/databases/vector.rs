use anyhow::Result;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollection, Distance as QDistance, VectorParams, VectorsConfig,
    vectors_config::Config,
};
use tracing::info;

/// Vector store for news, claims, and resolutions
/// Uses Qdrant for semantic search
pub struct VectorStore {
    client: Qdrant,
}

impl VectorStore {
    pub async fn new(url: &str) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;

        info!("✅ Connected to Qdrant vector store");

        Ok(Self { client })
    }

    pub async fn init_collections(&self) -> Result<()> {
        // Create collection for news articles
        self.client
            .create_collection(CreateCollection {
                collection_name: "news".to_string(),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: 768, // Embedding dimension
                        distance: QDistance::Cosine.into(),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await?;

        // Create collection for market claims
        self.client
            .create_collection(CreateCollection {
                collection_name: "claims".to_string(),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: 768,
                        distance: QDistance::Cosine.into(),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await?;

        info!("✅ Initialized vector store collections");

        Ok(())
    }

    pub async fn index_news(&self, article: &NewsArticle) -> Result<()> {
        // TODO: Generate embeddings and store
        Ok(())
    }

    pub async fn search_similar(&self, query: &str) -> Result<Vec<NewsArticle>> {
        // TODO: Semantic search
        Ok(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct NewsArticle {
    pub id: String,
    pub url: String,
    pub title: String,
    pub content: String,
    pub themes: Vec<String>,
    pub tone: f64,
}
