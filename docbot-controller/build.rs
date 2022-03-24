use docbot_crd::DeploymentHook;
use kube::CustomResourceExt;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let crate_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let schema = serde_yaml::to_string(&DeploymentHook::crd()).unwrap();
    let crd_schema_path = Path::new(&crate_dir)
        .join("..")
        .join("deploymenthooks.apps.mx.com.yaml");
    let mut f = File::create(&crd_schema_path).unwrap();
    f.write_all(schema.as_bytes()).unwrap();
}
