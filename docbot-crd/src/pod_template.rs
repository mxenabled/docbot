use crate::DeploymentHook;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::PodTemplate;
use kube::{api::ListParams, client::Client, Api};
use lru::LruCache;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

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
        todo!()
    }
}
