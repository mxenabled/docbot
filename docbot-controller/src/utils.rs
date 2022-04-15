use k8s_openapi::api::apps::v1::Deployment;

pub trait DeploymentStatusUtil {
    fn did_successfully_deploy(&self) -> bool;
}

impl DeploymentStatusUtil for Deployment {
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
}
