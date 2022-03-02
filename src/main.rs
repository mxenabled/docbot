use crate::cache::DeploymentHookCache;
use crd::DeploymentHook;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::apps::v1::{Deployment, DeploymentStatus};
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::PodTemplateSpec;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{ListParams, PostParams},
    client::Client,
    core::WatchEvent,
    Api,
};
use kube_runtime::controller::{Context, ReconcilerAction};

mod cache;
mod crd;
mod job;

async fn create_job_for_deployment_hook(
    client: Client,
    hook: &DeploymentHook,
) -> Result<(), Box<dyn std::error::Error>> {
    let generated_job =
        job::generate_from_template(hook, hook.get_pod_template(client.clone()).await?)?;

    let job_api: Api<Job> = Api::namespaced(
        client.clone(),
        &generated_job.metadata.namespace.as_ref().unwrap(),
    );

    println!(
        "------- JOB:\n{}",
        serde_yaml::to_string(&generated_job).unwrap()
    );

    job_api
        .create(&PostParams::default(), &generated_job)
        .await?;

    Ok(())
}

async fn watch_for_new_deployments(
    client: Client,
    cache: DeploymentHookCache,
) -> Result<(), Box<dyn std::error::Error>> {
    let deployment_api: Api<Deployment> = Api::all(client.clone());
    let params = ListParams::default().labels("apps.mx.com/deploymenthook");

    let resource_version = deployment_api
        .list(&params)
        .await?
        .metadata
        .resource_version
        .expect("invalid call");

    println!(
        "Current Deployment API ResrouceVersion: {}, Subscribing...",
        &resource_version
    );

    let mut stream = deployment_api
        .watch(&params, &resource_version)
        .await?
        .boxed();

    while let Some(event) = stream.try_next().await? {
        match event {
            WatchEvent::Added(deployment) | WatchEvent::Modified(deployment) => {
                // Check to see if the deployment has finished
                if let (Some(status), Some(spec)) =
                    (deployment.status.as_ref(), deployment.spec.as_ref())
                {
                    if let (Some(ready_replicas), Some(replicas), Some(deployment_replicas)) =
                        (status.ready_replicas, status.replicas, spec.replicas)
                    {
                        // println!("DEPLOYMENT STATUS: {:?}", deployment);
                        // I think we just need to make sure these two values match in order for
                        // this to be consider a completed deployment.
                        if ready_replicas == replicas && replicas == deployment_replicas {
                            for hook in cache.find_by_matching_deployment(&deployment).iter() {
                                create_job_for_deployment_hook(client.clone(), hook).await?;
                            }
                        }
                    }
                }
            }
            _ => { /* ignore */ }
        }
    }

    Ok(())

    //     let deployments = deployment_api.list(&params).await?.items;
    //
    //     for deployment in deployments.iter() {
    //         println!("DEPLOYMENT: {:?}", deployment.metadata.labels);
    //
    //         for hook in cache.find_by_matching_deployment(deployment).iter() {
    //             create_job_for_deployment_hook(client.clone(), hook).await?;
    //         }
    //     }
    //
    //     Ok(())
}
// async fn referesh_deployment_hook_cache(hook_cache: Arc<Mutex<DeploymentHookCache>>) {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG environment variable.");

    // Prime the deployhook cache
    let cache = cache::DeploymentHookCache::default();
    cache.refresh(client.clone()).await?;

    // Refresh the cache every minute
    tokio::spawn({
        let cache = cache.clone();
        let client = client.clone();

        async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;

                if let Err(err) = cache.refresh(client.clone()).await {
                    println!("Failed to refresh the deployment hooks cache: {:?}", err);
                }
            }
        }
    });

    // Watch for deployment changes
    loop {
        watch_for_new_deployments(client.clone(), cache.clone()).await?;

        println!("Deployment watcher finished or expired, restarting...");
    }
}
