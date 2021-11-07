use serde::{Deserialize, Serialize};
use worker::kv::{KvError, KvStore, KvValue};

pub struct RatingAlarms {
    store: KvStore,
}

#[derive(Serialize, Deserialize)]
pub struct RatingSubscription {
    pub target: String,
    pub rating: u64,
}

impl RatingAlarms {
    pub fn setup(store: KvStore) -> Self {
        Self { store }
    }

    pub async fn all_subscribers(&self) -> Result<impl Iterator<Item = i64>, KvError> {
        self.store
            .list()
            .execute()
            .await
            .map(|res| res.keys.into_iter().filter_map(|key| key.name.parse().ok()))
    }

    pub async fn get_subscription(
        &self,
        subscriber: i64,
    ) -> Result<Option<RatingSubscription>, KvError> {
        self.store
            .get(&subscriber.to_string())
            .await
            .map(|option| option.and_then(|value| value.as_json().ok()))
    }

    pub async fn set_subscription(
        &self,
        subscriber: i64,
        target: impl Into<String>,
        rating: u64,
    ) -> worker::Result<()> {
        let json = serde_json::to_string(&RatingSubscription {
            target: target.into(),
            rating,
        })?;
        self.store
            .put(&subscriber.to_string(), json)?
            .execute()
            .await
            .map_err(Into::into)
    }

    pub async fn unsubscribe(&self, subscriber: i64) -> Result<(), KvError> {
        self.store.delete(&subscriber.to_string()).await
    }
}

pub struct ProfileImages {
    store: KvStore,
}

impl ProfileImages {
    pub fn setup(store: KvStore) -> Self {
        Self { store }
    }

    pub async fn get_id(&self, handle: &str) -> Result<Option<String>, KvError> {
        self.store
            .get(handle)
            .await
            .map(|value| value.map(KvValue::as_string))
    }

    pub async fn set_id(&self, handle: &str, file_id: &str) -> Result<(), KvError> {
        self.store
            .put(handle, file_id)?
            .expiration_ttl(60 * 60 * 24 * 7)
            .execute()
            .await
    }
}
