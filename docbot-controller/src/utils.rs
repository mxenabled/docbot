use k8s_openapi::api::apps::v1::Deployment;
use sha2::{Digest, Sha256};

pub trait DeploymentExt {
    fn did_successfully_deploy(&self) -> bool;

    fn pod_template_hash(&self) -> Option<String>;
}

impl DeploymentExt for Deployment {
    fn did_successfully_deploy(&self) -> bool {
        // Check to see if the deployment has finished
        if let (Some(status), Some(spec)) = (self.status.as_ref(), self.spec.as_ref()) {
            if let (Some(ready_replicas), Some(replicas), Some(deployment_replicas)) =
                (status.ready_replicas, status.replicas, spec.replicas)
            {
                // println!("DEPLOYMENT STATUS: {:?}", deployment);
                // I think we just need to make sure these two values match in order for
                // this to be consider a completed deployment.
                return ready_replicas == replicas && replicas == deployment_replicas;
            }
        }

        return false;
    }

    fn pod_template_hash(&self) -> Option<String> {
        if let Some(spec) = self.spec.as_ref() {
            if let Some(ref pod_spec) = spec.template.spec {
                let payload = serde_yaml::to_string(pod_spec).expect("will always be valid");

                return Some(format!("{:X}", Sha256::digest(payload)));
            }
        }

        None
    }
}
