use std::fs;
use std::path::Path;

use schemars::schema_for;
use zed_interfaces::environment_lock::EnvironmentLock;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = Path::new("schemas/environment-lock-v1.json");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let schema = schema_for!(EnvironmentLock);
    let json = serde_json::to_string_pretty(&schema)? + "\n";
    fs::write(output, json)?;
    Ok(())
}
