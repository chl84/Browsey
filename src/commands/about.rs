use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutBuildInfo {
    pub profile: &'static str,
    pub target_os: &'static str,
    pub target_arch: &'static str,
    pub target_family: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub app_name: &'static str,
    pub version: &'static str,
    pub changelog: &'static str,
    pub license: &'static str,
    pub third_party_notices: &'static str,
    pub build: AboutBuildInfo,
}

#[tauri::command]
pub fn about_info() -> AboutInfo {
    AboutInfo {
        app_name: "Browsey",
        version: env!("CARGO_PKG_VERSION"),
        changelog: include_str!("../../CHANGELOG.md"),
        license: include_str!("../../LICENSE"),
        third_party_notices: include_str!("../../THIRD_PARTY_NOTICES"),
        build: AboutBuildInfo {
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            target_os: std::env::consts::OS,
            target_arch: std::env::consts::ARCH,
            target_family: std::env::consts::FAMILY,
        },
    }
}
