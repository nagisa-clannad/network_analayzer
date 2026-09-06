import { invoke } from "@tauri-apps/api/core";

export type ApiError = { code: string; message: string };
export type ApiEnvelope<T> = { ok: boolean; data?: T; error?: ApiError };
export type ScopeKind = "cidr" | "single_ip" | "seed_device" | "site_context";
export type ProviderCapabilityState = "available" | "no_privilege" | "unsupported";

export type ProviderCapabilities = {
  icmp: ProviderCapabilityState;
  arpNdp: ProviderCapabilityState;
  route: ProviderCapabilityState;
  localNetwork: ProviderCapabilityState;
  interfaceEnumeration: ProviderCapabilityState;
  rawPacket: ProviderCapabilityState;
};

export type LocalNetworkPreflight = {
  providerId: string;
  providerVersion: string;
  os: string;
  capabilities: ProviderCapabilities;
  interfaceCount: number;
  interfaces: Array<{ name: string; kind: string; adapterName?: string; ips: string[]; physicalPortState: string }>;
  diagnostics: string[];
};

export type PreflightReport = {
  scopeTarget: string;
  scopeKind: ScopeKind;
  activeProbeEnabled: boolean;
  maxHosts: number;
  estimatedTargets: number;
  capabilities: ProviderCapabilities;
  skippedFeatures: string[];
};

function desktopUnavailable<T>(): ApiEnvelope<T> {
  return { ok: false, error: { code: "desktop_backend_unavailable", message: "Tauri desktop backend に接続されていません。検証は実行されませんでした。" } };
}

export async function validateScope(kind: ScopeKind, target: string): Promise<ApiEnvelope<{ valid: boolean; readOnly: boolean }>> {
  if (!("__TAURI_INTERNALS__" in window)) return desktopUnavailable();
  return invoke<ApiEnvelope<{ valid: boolean; readOnly: boolean }>>("validate_scope", { request: { kind, target } });
}

export async function initializeProject(displayName: string, scopeKind: ScopeKind, scopeTarget: string): Promise<ApiEnvelope<{ projectId: string }>> {
  if (!("__TAURI_INTERNALS__" in window)) return desktopUnavailable();
  return invoke<ApiEnvelope<{ projectId: string }>>("initialize_project", { request: { displayName, scopeKind, scopeTarget } });
}

export async function getDefaultProfile(projectId: string): Promise<ApiEnvelope<{ profileId: string }>> {
  if (!("__TAURI_INTERNALS__" in window)) return desktopUnavailable();
  return invoke<ApiEnvelope<{ profileId: string }>>("get_default_profile", { request: { projectId } });
}

export async function scanPreflight(profileId: string): Promise<ApiEnvelope<PreflightReport>> {
  if (!("__TAURI_INTERNALS__" in window)) return desktopUnavailable();
  return invoke<ApiEnvelope<PreflightReport>>("scan_preflight", { request: { profileId } });
}

export async function preflightLocalNetwork(): Promise<ApiEnvelope<LocalNetworkPreflight>> {
  if (!("__TAURI_INTERNALS__" in window)) return desktopUnavailable();
  return invoke<ApiEnvelope<LocalNetworkPreflight>>("preflight_local_network");
}
