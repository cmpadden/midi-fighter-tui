use crate::protocol::ConfigSnapshot;

#[allow(dead_code)]
pub fn export_snapshot_json(snapshot: &ConfigSnapshot) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(snapshot)
}

#[allow(dead_code)]
pub fn export_snapshot_toml(snapshot: &ConfigSnapshot) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(snapshot)
}
