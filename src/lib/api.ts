// Thin wrapper around Tauri's `invoke()` so every call is typed against
// the Rust command signatures in src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";
import type {
  DiscoveryResult,
  HelperStatus,
  HelperVersionInfo,
  InstallEaResult,
  PairedEvent,
} from "./types";

export const api = {
  status: () => invoke<HelperStatus>("status"),
  startPairing: () => invoke<void>("start_pairing"),
  pairWithCode: (code: string) =>
    invoke<PairedEvent>("pair_with_code", { code }),
  discoverMt5: () => invoke<DiscoveryResult>("discover_mt5"),
  isMt5Running: () => invoke<boolean>("is_mt5_running"),
  installEa: () => invoke<InstallEaResult>("install_ea"),
  launchMt5: () => invoke<void>("launch_mt5"),
  rotateKey: () => invoke<string>("rotate_key"),
  logTelemetry: (phase: string, error?: string) =>
    invoke<void>("log_telemetry", { phase, error }),
  getHelperVersion: () => invoke<HelperVersionInfo>("get_helper_version"),
};
