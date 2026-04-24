// Shapes that mirror the Rust command return types in
// `src-tauri/src/commands.rs`. Keep these in sync on every change.

export interface HelperStatus {
  paired: boolean;
  apiKeyPrefix: string | null;
  version: string;
}

export interface Terminal {
  data_dir: string;
  experts_dir: string;
  config_dir: string;
  profiles_dir: string;
  broker_build: string;
}

export interface DiscoveryResult {
  terminals: Terminal[];
  mt5_running: boolean;
}

export type ProfileWriteOutcome =
  | "Written"
  | "NeedsManualDrag"
  | { Skipped: { reason: string } };

export interface WhitelistSummary {
  terminal: string;
  edited: boolean;
  alreadyPresent: boolean;
  error: string | null;
}

export interface ProfileSummary {
  terminal: string;
  outcome: ProfileWriteOutcome;
}

export interface InstallEaResult {
  writtenTo: string[];
  whitelistResults: WhitelistSummary[];
  profileResults: ProfileSummary[];
}

export interface PairedEvent {
  userId: string;
  userEmail: string | null;
  apiKeyPrefix: string;
  brokerName: string;
}

export interface HelperVersionInfo {
  latest: string;
  minSupported: string | null;
  downloadUrls: {
    mac: string | null;
    windows: string | null;
  };
  releaseNotes: string;
}
