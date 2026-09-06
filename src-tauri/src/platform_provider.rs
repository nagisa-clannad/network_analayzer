//! Read-only local interface provider.
//!
//! This module deliberately does not execute a shell command, open a socket,
//! query DNS, or access credentials.  Route/neighbor/raw-packet providers are
//! separate capabilities and remain unsupported until their OS API adapters
//! have been reviewed.

use get_if_addrs::{get_if_addrs, IfAddr};

use crate::core_domain::{
    InterfaceKind, LocalAdapter, LocalInterface, LocalNetworkResult, PlatformNetworkProvider,
    ProviderCapabilities, ProviderCapabilityState,
};

pub struct RealPlatformNetworkProvider;

impl PlatformNetworkProvider for RealPlatformNetworkProvider {
    fn check_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            icmp: ProviderCapabilityState::Unsupported,
            arp_ndp: ProviderCapabilityState::Unsupported,
            route: ProviderCapabilityState::Unsupported,
            local_network: ProviderCapabilityState::Available,
            interface_enumeration: ProviderCapabilityState::Available,
            raw_packet: ProviderCapabilityState::Unsupported,
        }
    }

    fn collect_network_info(&self) -> Result<LocalNetworkResult, String> {
        collect_local_interfaces()
    }
}

/// A deterministic provider used only by contract tests.  It never performs I/O.
#[cfg(test)]
pub struct MockPlatformNetworkProvider {
    pub capabilities: ProviderCapabilities,
    pub result: Result<LocalNetworkResult, String>,
}

#[cfg(test)]
impl PlatformNetworkProvider for MockPlatformNetworkProvider {
    fn check_capabilities(&self) -> ProviderCapabilities { self.capabilities.clone() }
    fn collect_network_info(&self) -> Result<LocalNetworkResult, String> { self.result.clone() }
}

fn interface_kind(is_loopback: bool) -> InterfaceKind {
    if is_loopback { InterfaceKind::Loopback } else { InterfaceKind::Unknown }
}

fn safe_interface_name(name: &str) -> String {
    let cleaned: String = name.chars().filter(|character| !character.is_control()).take(128).collect();
    if cleaned.is_empty() { "unknown-interface".to_string() } else { cleaned }
}

fn collect_local_interfaces() -> Result<LocalNetworkResult, String> {
    let os_interfaces = get_if_addrs().map_err(|_| "interface_enumeration_failed".to_string())?;
    let mut adapters = Vec::new();
    let mut interfaces = Vec::new();

    for os_interface in os_interfaces {
        let name = safe_interface_name(&os_interface.name);
        let is_loopback = os_interface.is_loopback();
        let kind = interface_kind(is_loopback);
        let ip = match os_interface.addr {
            IfAddr::V4(address) => address.ip.to_string(),
            IfAddr::V6(address) => address.ip.to_string(),
        };

        if !adapters.iter().any(|adapter: &LocalAdapter| adapter.name == name) {
            adapters.push(LocalAdapter { name: name.clone(), kind: "unknown".to_string() });
        }
        if let Some(existing) = interfaces.iter_mut().find(|value: &&mut LocalInterface| value.name == name) {
            existing.ips.push(ip);
        } else {
            interfaces.push(LocalInterface {
                name: name.clone(),
                kind,
                adapter_name: Some(name),
                ips: vec![ip],
                physical_port_state: "not_observed".to_string(),
            });
        }
    }

    Ok(LocalNetworkResult { adapters, interfaces, routes: Vec::new(), neighbors: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_os_loopback_flag_can_produce_loopback_kind() {
        assert_eq!(interface_kind(true), InterfaceKind::Loopback);
        assert_eq!(interface_kind(false), InterfaceKind::Unknown);
    }

    #[test]
    fn adapter_and_physical_state_are_unknown_without_port_evidence() {
        let value = LocalInterface { name: "Ethernet".into(), kind: InterfaceKind::Unknown, adapter_name: Some("Ethernet".into()), ips: vec!["192.0.2.10".into()], physical_port_state: "not_observed".into() };
        assert_eq!(value.kind, InterfaceKind::Unknown);
        assert_eq!(value.physical_port_state, "not_observed");
    }

    #[test]
    fn capability_contract_does_not_overclaim_active_features() {
        let provider = RealPlatformNetworkProvider;
        let capabilities = provider.check_capabilities();
        assert_eq!(capabilities.interface_enumeration, ProviderCapabilityState::Available);
        assert_eq!(capabilities.local_network, ProviderCapabilityState::Available);
        assert_eq!(capabilities.route, ProviderCapabilityState::Unsupported);
        assert_eq!(capabilities.arp_ndp, ProviderCapabilityState::Unsupported);
        assert_eq!(capabilities.icmp, ProviderCapabilityState::Unsupported);
        assert_eq!(capabilities.raw_packet, ProviderCapabilityState::Unsupported);
    }

    #[test]
    fn mock_provider_can_represent_partial_failure_without_fabricated_data() {
        let provider = MockPlatformNetworkProvider {
            capabilities: ProviderCapabilities {
                icmp: ProviderCapabilityState::Unsupported,
                arp_ndp: ProviderCapabilityState::Unsupported,
                route: ProviderCapabilityState::Unsupported,
                local_network: ProviderCapabilityState::NoPrivilege,
                interface_enumeration: ProviderCapabilityState::NoPrivilege,
                raw_packet: ProviderCapabilityState::Unsupported,
            },
            result: Err("interface_enumeration_failed".into()),
        };
        assert!(provider.collect_network_info().is_err());
        assert_eq!(provider.check_capabilities().interface_enumeration, ProviderCapabilityState::NoPrivilege);
    }
}
