use worker::kv::{KvError, KvStore, KvValue};

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
