// Core contracts: no provider I/O or secret material is permitted here.
// Some contracts are not invoked until the provider/resolver phases are implemented.
#![allow(dead_code)]
use std::net::IpAddr;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryScopeKind { Cidr, SingleIp, SeedDevice, SiteContext }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryScope { pub id: Uuid, pub kind: DiscoveryScopeKind, pub target: String, pub enabled: bool }
impl DiscoveryScope {
    pub fn validate(&self) -> Result<(), DomainError> {
        let target = self.target.trim();
        if target.is_empty() || target.len() > 255 { return Err(DomainError::InvalidScope); }
        match self.kind {
            DiscoveryScopeKind::Cidr => {
                let Some((address, prefix)) = target.split_once('/') else { return Err(DomainError::InvalidScope); };
                let Ok(address) = address.parse::<IpAddr>() else { return Err(DomainError::InvalidScope); };
                let Ok(prefix) = prefix.parse::<u8>() else { return Err(DomainError::InvalidScope); };
                if prefix <= if address.is_ipv4() { 32 } else { 128 } { Ok(()) } else { Err(DomainError::InvalidScope) }
            }
            DiscoveryScopeKind::SingleIp if target.parse::<IpAddr>().is_err() => Err(DomainError::InvalidScope),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProfile {
    pub id: Uuid, pub name: String, pub scope_ids: Vec<Uuid>,
    /// Secure-store references only; secrets do not enter this model.
    pub credential_reference_ids: Vec<Uuid>, pub read_only: bool, pub active_probe_enabled: bool,
}
impl ScanProfile { pub fn validate(&self) -> Result<(), DomainError> {
    if self.name.trim().is_empty() || self.name.len() > 120 || !self.read_only || self.active_probe_enabled { Err(DomainError::UnsafeProfile) } else { Ok(()) }
} }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanSessionState { Requested, Preflight, Running, Completed, Failed, Cancelled, Interrupted, Rejected }
impl ScanSessionState { pub fn can_transition_to(self, next: Self) -> bool {
    use ScanSessionState::*;
    matches!((self, next), (Requested, Preflight) | (Requested, Rejected) | (Preflight, Running) | (Preflight, Rejected) | (Running, Completed) | (Running, Failed) | (Running, Cancelled) | (Running, Interrupted))
} }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssertionState { Known, Inferred, Unknown, UserConfirmed }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence { pub id: Uuid, pub target_entity_id: Uuid, pub attribute: String, pub source_type: String, pub confidence: f32, pub assertion_state: AssertionState }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceKind { Physical, Virtual, Vlan, Svi, Bridge, Bond, Lag, Tunnel, Loopback, Unknown }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceEndpoint { pub interface_id: Uuid, pub kind: InterfaceKind, pub has_physical_port_evidence: bool }
impl InterfaceEndpoint { pub fn is_physical_link_endpoint(&self) -> bool { self.kind == InterfaceKind::Physical && self.has_physical_port_evidence } }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalLinkDraft { pub source: InterfaceEndpoint, pub target: InterfaceEndpoint, pub evidence_ids: Vec<Uuid> }
impl PhysicalLinkDraft { pub fn validate(&self) -> Result<(), DomainError> {
    if self.source.interface_id == self.target.interface_id { return Err(DomainError::SelfPhysicalLink); }
    if !self.source.is_physical_link_endpoint() || !self.target.is_physical_link_endpoint() { return Err(DomainError::LogicalInterfaceCannotBePhysicalLinkEndpoint); }
    if self.evidence_ids.is_empty() { return Err(DomainError::PhysicalLinkNeedsEvidence); }
    Ok(())
} }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapabilityState {
    Available,
    NoPrivilege,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub icmp: ProviderCapabilityState,
    pub arp_ndp: ProviderCapabilityState,
    pub route: ProviderCapabilityState,
    pub local_network: ProviderCapabilityState,
    pub interface_enumeration: ProviderCapabilityState,
    pub raw_packet: ProviderCapabilityState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub scope_target: String,
    pub scope_kind: DiscoveryScopeKind,
    pub active_probe_enabled: bool,
    pub max_hosts: usize,
    pub estimated_targets: usize,
    pub capabilities: ProviderCapabilities,
    pub skipped_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalAdapter {
    pub name: String,
    pub kind: String, // "physical" | "virtual" | "unknown"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalInterface {
    pub name: String,
    pub kind: InterfaceKind,
    pub adapter_name: Option<String>,
    pub ips: Vec<String>,
    pub physical_port_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalRoute {
    pub destination: String,
    pub next_hop: Option<String>,
    pub interface_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalNeighbor {
    pub ip: String,
    pub mac: String,
    pub interface_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalNetworkResult {
    pub adapters: Vec<LocalAdapter>,
    pub interfaces: Vec<LocalInterface>,
    pub routes: Vec<LocalRoute>,
    pub neighbors: Vec<LocalNeighbor>,
}

pub trait PlatformNetworkProvider {
    fn check_capabilities(&self) -> ProviderCapabilities;
    fn collect_network_info(&self) -> Result<LocalNetworkResult, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalNetworkPreflight {
    pub provider_id: String,
    pub provider_version: String,
    pub os: String,
    pub capabilities: ProviderCapabilities,
    pub interface_count: usize,
    pub interfaces: Vec<LocalInterface>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("scope is empty or does not match its kind")] InvalidScope,
    #[error("profile must be read-only and cannot enable active probes")] UnsafeProfile,
    #[error("a physical link cannot point to itself")] SelfPhysicalLink,
    #[error("a logical interface cannot be physical-link endpoint")] LogicalInterfaceCannotBePhysicalLinkEndpoint,
    #[error("a physical link needs retained evidence")] PhysicalLinkNeedsEvidence,
}
#[cfg(test)] mod tests { use super::*;
    #[test] fn virtual_endpoint_is_rejected() { let link = PhysicalLinkDraft { source: InterfaceEndpoint { interface_id: Uuid::new_v4(), kind: InterfaceKind::Physical, has_physical_port_evidence: true }, target: InterfaceEndpoint { interface_id: Uuid::new_v4(), kind: InterfaceKind::Vlan, has_physical_port_evidence: false }, evidence_ids: vec![Uuid::new_v4()] }; assert!(matches!(link.validate(), Err(DomainError::LogicalInterfaceCannotBePhysicalLinkEndpoint))); }
    #[test] fn completed_session_is_not_reopened() { assert!(!ScanSessionState::Completed.can_transition_to(ScanSessionState::Running)); }
    #[test] fn profile_cannot_enable_active_probes_or_embed_an_unsafe_mode() {
        let profile = ScanProfile { id: Uuid::new_v4(), name: "safe".into(), scope_ids: vec![], credential_reference_ids: vec![], read_only: true, active_probe_enabled: true };
        assert!(matches!(profile.validate(), Err(DomainError::UnsafeProfile)));
    }
    #[test] fn evidence_keeps_its_assertion_state_explicit() {
        let evidence = Evidence { id: Uuid::new_v4(), target_entity_id: Uuid::new_v4(), attribute: "physical_port".into(), source_type: "fixture".into(), confidence: 1.0, assertion_state: AssertionState::Known };
        assert_eq!(evidence.assertion_state, AssertionState::Known);
    }
}
