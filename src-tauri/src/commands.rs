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
    let name = request.display_name.trim(); if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) { return ApiEnvelope::failure("invalid_project_name", "Project name must be 1 to 120 characters and contain no control characters."); }
    let scope = DiscoveryScope { id: Uuid::now_v7(), kind: request.scope_kind, target: request.scope_target, enabled: true };
    if scope.validate().is_err() { return ApiEnvelope::failure("invalid_scope", "A valid explicit Discovery Scope is required."); }
    let mut database = match state.database.lock() { Ok(value) => value, Err(_) => return ApiEnvelope::failure("state_unavailable", "Application state is unavailable.") };
    let scope_kind = match &scope.kind {
        DiscoveryScopeKind::Cidr => "cidr",
        DiscoveryScopeKind::SingleIp => "single_ip",
        DiscoveryScopeKind::SeedDevice => "seed_device",
        DiscoveryScopeKind::SiteContext => "site_context",
    };
    match database.create_project(name, scope_kind, &scope.target) { Ok(project_id) => ApiEnvelope::success(ProjectCreated { project_id }), Err(_) => ApiEnvelope::failure("storage_error", "Project could not be stored.") }
}
#[derive(Debug, Deserialize)] #[serde(rename_all = "camelCase")] pub struct ValidateScopeRequest { pub kind: DiscoveryScopeKind, pub target: String }
#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")] pub struct ScopeValidation { pub valid: bool, pub read_only: bool }
#[tauri::command] pub fn validate_scope(request: ValidateScopeRequest) -> ApiEnvelope<ScopeValidation> {
    match (DiscoveryScope { id: Uuid::now_v7(), kind: request.kind, target: request.target, enabled: true }).validate() { Ok(()) => ApiEnvelope::success(ScopeValidation { valid: true, read_only: true }), Err(_) => ApiEnvelope::failure("invalid_scope", "Scope must be non-empty and match its selected kind.") }
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
    if Uuid::parse_str(&request.project_id).is_err() {
        return ApiEnvelope::failure("invalid_project_id", "Project ID must be a UUID.");
    }
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
    if Uuid::parse_str(&request.profile_id).is_err() {
        return ApiEnvelope::failure("invalid_profile_id", "Profile ID must be a UUID; no scan was started.");
    }
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

    if profile.validate().is_err()
        || scopes.is_empty()
        || scopes.iter().any(|scope| scope.validate().is_err())
        || profile.scope_ids.len() != scopes.len()
        || profile.scope_ids.iter().any(|id| !scopes.iter().any(|scope| scope.id == *id))
    {
        return ApiEnvelope::failure("invalid_profile", "The stored profile or scope is not safe to run. No scan was started.");
    }
    if scopes.len() > 1 {
        return ApiEnvelope::failure("multiple_scopes_unsupported", "This foundation Preflight supports one explicit Scope at a time. No scan was started.");
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
    if matches!(capabilities.icmp, crate::core_domain::ProviderCapabilityState::Unsupported) {
        skipped_features.push("ICMP Echo (Ping) is unsupported by the installed provider.".to_string());
    }
    if matches!(capabilities.arp_ndp, crate::core_domain::ProviderCapabilityState::Unsupported) {
        skipped_features.push("ARP/NDP collection is unsupported by the installed provider.".to_string());
    }
    if matches!(capabilities.route, crate::core_domain::ProviderCapabilityState::Unsupported) {
        skipped_features.push("Route collection is unsupported by the installed provider.".to_string());
    }
    if matches!(capabilities.raw_packet, crate::core_domain::ProviderCapabilityState::Unsupported) {
        skipped_features.push("Raw packet capture is unsupported by the installed provider.".to_string());
    }
    if profile.active_probe_enabled {
        skipped_features.push("Active port sweeps are disabled because the profile prevents active probes.".to_string());
    }

    let max_hosts = PREFLIGHT_MAX_HOSTS;

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
            adapters: result.adapters,
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

const PREFLIGHT_MAX_HOSTS: usize = 1024;

fn estimate_targets(target: &str, kind: &crate::core_domain::DiscoveryScopeKind) -> usize {
    match kind {
        crate::core_domain::DiscoveryScopeKind::Cidr => {
            if let Some((_, prefix)) = target.split_once('/') {
                if let Ok(p) = prefix.parse::<u8>() {
                    let address = target.split_once('/').and_then(|(value, _)| value.parse::<std::net::IpAddr>().ok());
                    let host_bits = match address {
                        Some(std::net::IpAddr::V4(_)) if p <= 32 => 32_u32 - p as u32,
                        Some(std::net::IpAddr::V6(_)) if p <= 128 => 128_u32 - p as u32,
                        _ => return 0,
                    };
                    // Preflight only needs to distinguish <= max from over-limit. Cap
                    // before shifting so /0 and broad IPv6 prefixes cannot overflow.
                    let estimate = if host_bits >= usize::BITS {
                        usize::MAX
                    } else {
                        1_usize << host_bits
                    };
                    return estimate.min(PREFLIGHT_MAX_HOSTS + 1);
                }
            }
            0
        }
        crate::core_domain::DiscoveryScopeKind::SingleIp => 1,
        crate::core_domain::DiscoveryScopeKind::SeedDevice => 1,
        crate::core_domain::DiscoveryScopeKind::SiteContext => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::estimate_targets;
    use crate::core_domain::DiscoveryScopeKind;

    #[test]
    fn estimate_targets_is_family_aware_and_capped() {
        assert_eq!(estimate_targets("2001:db8::/32", &DiscoveryScopeKind::Cidr), 1025);
        assert_eq!(estimate_targets("192.0.2.0/24", &DiscoveryScopeKind::Cidr), 256);
        assert_eq!(estimate_targets("192.0.2.0/0", &DiscoveryScopeKind::Cidr), 1025);
        assert_eq!(estimate_targets("2001:db8::/128", &DiscoveryScopeKind::Cidr), 1);
    }
}
