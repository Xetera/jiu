use lapin::options::{
    BasicPublishOptions, ExchangeBindOptions, ExchangeDeclareOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind, Result as LapinResult,
};
use log::error;

use crate::dispatcher::dispatcher::DispatchablePayload;

pub struct AMQPDispatcher {
    channel: Channel,
}

const DIRECT_QUEUE_NAME: &str = "image_discovery";

impl AMQPDispatcher {
    pub async fn from_connection_string(url: &str) -> LapinResult<Self> {
        let conn = Connection::connect(url, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await?;
        channel
            .exchange_declare(
                DIRECT_QUEUE_NAME,
                ExchangeKind::Topic,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        // technically we're only a publisher and shouldn't be
        // declaring a queue but whatever
        channel
            .queue_declare(
                DIRECT_QUEUE_NAME,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        channel
            .exchange_bind(
                DIRECT_QUEUE_NAME,
                DIRECT_QUEUE_NAME,
                DIRECT_QUEUE_NAME,
                ExchangeBindOptions::default(),
                FieldTable::default(),
            )
            .await?;
        LapinResult::Ok(Self { channel })
    }
    pub async fn publish(&self, payload: &DispatchablePayload) {
        match serde_json::to_vec(&payload) {
            Err(err) => {
                error!("Error serializing AMQP payload {:?}", err)
            }
            Ok(value) => {
                let result = self
                    .channel
                    .basic_publish(
                        "",
                        DIRECT_QUEUE_NAME,
                        BasicPublishOptions::default(),
                        value,
                        BasicProperties::default(),
                    )
                    .await;
                if let Err(e) = result {
                    error!("Couldn't publish to AMQP {:?}", e)
                }
            }
        }
    }
}
