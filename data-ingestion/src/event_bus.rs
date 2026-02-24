use anyhow::Result;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use tracing::{debug, error};

use common::MarketEvent;
use std::time::Duration;

pub struct KafkaProducer {
    producer: FutureProducer,
}

impl KafkaProducer {
    pub async fn new(brokers: &str) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        Ok(Self { producer })
    }

    pub async fn publish(&self, topic: &str, event: &MarketEvent) -> Result<()> {
        let key = event.market_id().to_string();
        let value = serde_json::to_string(event)?;

        debug!("Publishing to {}: {:?}", topic, event);

        self.producer
            .send(
                FutureRecord::to(topic).key(&key).payload(&value),
                Timeout::After(Duration::from_secs(5)),
            )
            .await
            .map_err(|(e, _)| {
                error!("Failed to publish to {}: {}", topic, e);
                anyhow::anyhow!("Failed to publish message: {}", e)
            })?;

        Ok(())
    }
}

