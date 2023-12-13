use futures::TryStreamExt;

use k8s_openapi::api::core::v1::PodTemplate;
use kube::{
    api::{ListParams, WatchEvent},
    client::Client,
    Api,
};
use lru::LruCache;

use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Clone)]
pub struct PodTemplateService {
    cache: Arc<Mutex<LruCache<(String, String), PodTemplate>>>,
    client: Client,
}

impl PodTemplateService {
    pub fn new(client: Client) -> Self {
        let cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap())));

        Self { cache, client }
    }

    pub async fn get(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<PodTemplate>, Box<dyn std::error::Error>> {
        let mut locked = self.cache.lock().await;

        // Check the LRU cache for the pod template.
        let cache_key = (namespace.to_string(), name.to_string());
        if let Some(pod_template) = locked.get(&cache_key) {
            return Ok(Some(pod_template.clone()));
        }

        // Otherwise we should pull directly from the API.
        let pod_template_api: Api<PodTemplate> = Api::namespaced(self.client.clone(), namespace);
        let pod_template = pod_template_api.get(name).await?;

        // Now fill the cache so we can avoid an API call next time.
        locked.push(cache_key, pod_template.clone());

        Ok(Some(pod_template))
    }

    pub async fn push(&self, pod_template: PodTemplate) {
        let namespace = pod_template
            .metadata
            .namespace
            .clone()
            .unwrap_or_else(|| "default".to_string());

        let mut locked = self.cache.lock().await;

        if let Some(name) = pod_template.metadata.name.clone() {
            let cache_key = (namespace.to_string(), name.to_string());

            locked.push(cache_key, pod_template);
        } else {
            warn!("Could not find a name for pod_template in namepsace: {namespace}")
        }
    }

    pub async fn watch_for_changes(&self) -> Result<(), Box<dyn std::error::Error>> {
        let pod_template_api: Api<PodTemplate> = Api::all(self.client.clone());

        loop {
            let lp = ListParams::default();
            let pod_template_stream = pod_template_api.watch(&lp, "0").await?;
            tokio::pin!(pod_template_stream);

            while let Some(pod_template_event) = pod_template_stream.try_next().await? {
                match pod_template_event {
                    WatchEvent::Added(pod_template) | WatchEvent::Modified(pod_template) => {
                        let name = pod_template
                            .metadata
                            .name
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string());
                        let namespace = pod_template
                            .metadata
                            .namespace
                            .clone()
                            .unwrap_or_else(|| "default".to_string());

                        info!(
                            "Witnessed {:?} event for PodTeamplte: {}/{}",
                            pod_template_event, name, namespace
                        );
                        self.push(pod_template).await;
                    }
                    _ => { /* ignore */ }
                }
            }
        }
    }
}
