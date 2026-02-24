use anyhow::Result;
use neo4rs::{Graph, query};
use tracing::info;

/// Graph database for people, organizations, and event dependencies
/// Uses Neo4j for relationship mapping
pub struct GraphDB {
    graph: Graph,
}

impl GraphDB {
    pub fn new(uri: &str, user: &str, password: &str) -> Result<Self> {
        let graph = Graph::new(uri, user, password)?;

        info!("✅ Connected to Neo4j graph database");

        Ok(Self { graph })
    }

    pub async fn init_schema(&self) -> Result<()> {
        // Create indexes for better query performance
        self.graph
            .execute(query(
                "CREATE INDEX person_name IF NOT EXISTS FOR (p:Person) ON (p.name)",
            ))
            .await?;

        self.graph
            .execute(query(
                "CREATE INDEX org_name IF NOT EXISTS FOR (o:Organization) ON (o.name)",
            ))
            .await?;

        self.graph
            .execute(query(
                "CREATE INDEX market_id IF NOT EXISTS FOR (m:Market) ON (m.id)",
            ))
            .await?;

        info!("✅ Initialized graph database schema");

        Ok(())
    }

    pub async fn add_person(&self, name: &str, source: &str) -> Result<()> {
        self.graph
            .execute(
                query("MERGE (p:Person {name: $name}) SET p.source = $source")
                    .param("name", name)
                    .param("source", source),
            )
            .await?;

        Ok(())
    }

    pub async fn add_organization(&self, name: &str, source: &str) -> Result<()> {
        self.graph
            .execute(
                query(
                    "MERGE (o:Organization {name: $name}) SET o.source = $source",
                )
                .param("name", name)
                .param("source", source),
            )
            .await?;

        Ok(())
    }

    pub async fn link_person_to_market(
        &self,
        person: &str,
        market_id: &str,
        relation: &str,
    ) -> Result<()> {
        self.graph
            .execute(
                query(
                    "
                MATCH (p:Person {name: $person})
                MATCH (m:Market {id: $market_id})
                MERGE (p)-[r:RELATED {type: $relation}]->(m)
                ",
                )
                .param("person", person)
                .param("market_id", market_id)
                .param("relation", relation),
            )
            .await?;

        Ok(())
    }
}
