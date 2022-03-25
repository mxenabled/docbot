use docbot_crd::DeploymentHook;
use kube::CustomResourceExt;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let mut crd = DeploymentHook::crd();
    // NOTE: The namespace "default" is expected for a namespaced CRD?
    crd.metadata.namespace = Some("default".into());

    // Write to file.
    let schema = serde_yaml::to_string(&crd).unwrap();
    let crate_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let crd_schema_path = Path::new(&crate_dir)
        .join("..")
        .join("deploymenthooks.apps.mx.com.yaml");
    let mut f = File::create(&crd_schema_path).unwrap();
    f.write_all(schema.as_bytes()).unwrap();
}
