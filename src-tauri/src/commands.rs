use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use crate::{database::InventoryRow, core_domain::{DiscoveryScope, DiscoveryScopeKind}, AppState};

#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ApiError { pub code: &'static str, pub message: String }
#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ApiEnvelope<T: Serialize> { pub ok: bool, pub data: Option<T>, pub error: Option<ApiError> }
impl<T: Serialize> ApiEnvelope<T> { fn success(data: T) -> Self { Self { ok: true, data: Some(data), error: None } } fn failure(code: &'static str, message: impl Into<String>) -> Self { Self { ok: false, data: None, error: Some(ApiError { code, message: message.into() }) } } }
#[derive(Debug, Deserialize)] #[serde(rename_all = "camelCase")] pub struct InitializeProjectRequest { pub display_name: String, pub scope_kind: DiscoveryScopeKind, pub scope_target: String }
#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")] pub struct ProjectCreated { pub project_id: String }
#[tauri::command] pub fn initialize_project(state: State<'_, AppState>, request: InitializeProjectRequest) -> ApiEnvelope<ProjectCreated> {
    let name = request.display_name.trim(); if name.is_empty() || name.len() > 120 { return ApiEnvelope::failure("invalid_project_name", "Project name must be 1 to 120 characters."); }
    let scope = DiscoveryScope { id: Uuid::new_v4(), kind: request.scope_kind, target: request.scope_target, enabled: true };
    if scope.validate().is_err() { return ApiEnvelope::failure("invalid_scope", "A valid explicit Discovery Scope is required."); }
    let mut database = match state.database.lock() { Ok(value) => value, Err(_) => return ApiEnvelope::failure("state_unavailable", "Application state is unavailable.") };
    let scope_kind = serde_json::to_string(&scope.kind).unwrap_or_else(|_| "\"cidr\"".to_string()).trim_matches('"').to_string();
    match database.create_project(name, &scope_kind, &scope.target) { Ok(project_id) => ApiEnvelope::success(ProjectCreated { project_id }), Err(_) => ApiEnvelope::failure("storage_error", "Project could not be stored.") }
}
#[derive(Debug, Deserialize)] #[serde(rename_all = "camelCase")] pub struct ValidateScopeRequest { pub kind: DiscoveryScopeKind, pub target: String }
#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")] pub struct ScopeValidation { pub valid: bool, pub read_only: bool }
#[tauri::command] pub fn validate_scope(request: ValidateScopeRequest) -> ApiEnvelope<ScopeValidation> {
    match (DiscoveryScope { id: Uuid::new_v4(), kind: request.kind, target: request.target, enabled: true }).validate() { Ok(()) => ApiEnvelope::success(ScopeValidation { valid: true, read_only: true }), Err(_) => ApiEnvelope::failure("invalid_scope", "Scope must be non-empty and match its selected kind.") }
}
#[derive(Debug, Deserialize)] #[serde(rename_all = "camelCase")] pub struct ScanRequest { pub profile_id: String }
#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")] pub struct ScanRejected { pub state: &'static str, pub provider_status: &'static str }
#[tauri::command] pub fn request_scan(request: ScanRequest) -> ApiEnvelope<ScanRejected> {
    if Uuid::parse_str(&request.profile_id).is_err() { return ApiEnvelope::failure("invalid_profile_id", "Profile ID must be a UUID; no scan was started."); }
    ApiEnvelope::failure("provider_not_implemented", "No discovery provider is installed. No network request, credential lookup, or scan was started.")
}
#[tauri::command] pub fn list_inventory(state: State<'_, AppState>) -> ApiEnvelope<Vec<InventoryRow>> {
    let database = match state.database.lock() { Ok(value) => value, Err(_) => return ApiEnvelope::failure("state_unavailable", "Application state is unavailable.") };
    match database.inventory() { Ok(rows) => ApiEnvelope::success(rows), Err(_) => ApiEnvelope::failure("storage_error", "Inventory projection could not be loaded.") }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDefaultProfileRequest {
    pub project_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultProfileResponse {
    pub profile_id: String,
}

#[tauri::command]
pub fn get_default_profile(state: State<'_, AppState>, request: GetDefaultProfileRequest) -> ApiEnvelope<DefaultProfileResponse> {
    let database = match state.database.lock() {
        Ok(value) => value,
        Err(_) => return ApiEnvelope::failure("state_unavailable", "Application state is unavailable.")
    };

    match database.get_default_profile_id(&request.project_id) {
        Ok(Some(profile_id)) => ApiEnvelope::success(DefaultProfileResponse { profile_id }),
        Ok(None) => ApiEnvelope::failure("profile_not_found", "No scan profile found for this project."),
        Err(_) => ApiEnvelope::failure("storage_error", "Failed to query database for default profile."),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPreflightRequest {
    pub profile_id: String,
}

#[tauri::command]
pub fn scan_preflight(state: State<'_, AppState>, request: ScanPreflightRequest) -> ApiEnvelope<crate::core_domain::PreflightReport> {
    use crate::core_domain::PlatformNetworkProvider;
    let database = match state.database.lock() {
        Ok(value) => value,
        Err(_) => return ApiEnvelope::failure("state_unavailable", "Application state is unavailable.")
    };

    let result = match database.get_profile_and_scopes(&request.profile_id) {
        Ok(res) => res,
        Err(_) => return ApiEnvelope::failure("storage_error", "Failed to query database for scan profile.")
    };

    let Some((profile, scopes)) = result else {
        return ApiEnvelope::failure("profile_not_found", "Scan profile not found.");
    };

    if profile.validate().is_err() || scopes.iter().any(|scope| scope.validate().is_err()) {
        return ApiEnvelope::failure("invalid_profile", "The stored profile or scope is not safe to run. No scan was started.");
    }

    let mut estimated_targets = 0;
    let mut scope_target = String::new();
    let mut scope_kind = crate::core_domain::DiscoveryScopeKind::Cidr;

    if !scopes.is_empty() {
        scope_target = scopes[0].target.clone();
        scope_kind = scopes[0].kind.clone();
        
        for scope in &scopes {
            if scope.enabled {
                estimated_targets += estimate_targets(&scope.target, &scope.kind);
            }
        }
    }

    let provider = crate::platform_provider::RealPlatformNetworkProvider;
    let capabilities = provider.check_capabilities();

    let mut skipped_features = Vec::new();
    if matches!(capabilities.icmp, crate::core_domain::ProviderCapabilityState::NoPrivilege) {
        skipped_features.push("ICMP Echo (Ping) scan is skipped due to insufficient privileges.".to_string());
    }
    if profile.active_probe_enabled {
        skipped_features.push("Active port sweeps are disabled because the profile prevents active probes.".to_string());
    }

    let max_hosts = 1024;

    ApiEnvelope::success(crate::core_domain::PreflightReport {
        scope_target,
        scope_kind,
        active_probe_enabled: profile.active_probe_enabled,
        max_hosts,
        estimated_targets,
        capabilities,
        skipped_features,
    })
}

#[tauri::command]
pub fn preflight_local_network() -> ApiEnvelope<crate::core_domain::LocalNetworkPreflight> {
    use crate::core_domain::PlatformNetworkProvider;
    let provider = crate::platform_provider::RealPlatformNetworkProvider;
    let capabilities = provider.check_capabilities();
    match provider.collect_network_info() {
        Ok(result) => ApiEnvelope::success(crate::core_domain::LocalNetworkPreflight {
            provider_id: "local_network".to_string(),
            provider_version: "0.1.0".to_string(),
            os: std::env::consts::OS.to_string(),
            capabilities,
            interface_count: result.interfaces.len(),
            interfaces: result.interfaces,
            diagnostics: vec![
                "route_unsupported".to_string(),
                "arp_ndp_unsupported".to_string(),
                "icmp_unsupported".to_string(),
                "raw_packet_unsupported".to_string(),
            ],
        }),
        Err(_) => ApiEnvelope::failure("provider_unavailable", "Local interface enumeration was unavailable. No scan was started."),
    }
}

fn estimate_targets(target: &str, kind: &crate::core_domain::DiscoveryScopeKind) -> usize {
    match kind {
        crate::core_domain::DiscoveryScopeKind::Cidr => {
            if let Some((_, prefix)) = target.split_once('/') {
                if let Ok(p) = prefix.parse::<u8>() {
                    if p <= 32 {
                        let hosts = 2_usize.pow(32 - p as u32);
                        return hosts;
                    } else if p <= 128 {
                        return 65536; // IPv6
                    }
                }
            }
            0
        }
        crate::core_domain::DiscoveryScopeKind::SingleIp => 1,
        crate::core_domain::DiscoveryScopeKind::SeedDevice => 1,
        crate::core_domain::DiscoveryScopeKind::SiteContext => 1,
    }
}
