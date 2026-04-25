export interface SettingsPayload {
  toastDurationMs: number;
  toastPersistent: boolean;
  togglePanelShortcut: string;
  editor: string;
  autostartEnabled: boolean;
}

export interface SaveSettingsResult {
  restartRequired: boolean;
}

export interface CliInstallStatus {
  installed: boolean;
  pointsToCurrentExe: boolean;
  onPath: boolean;
  targetPath: string;
}

export interface CliInstallResult {
  targetPath: string;
  onPath: boolean;
  replacedExisting: boolean;
}

// All current settings are applied live by the backend (toast, shortcut,
// update flag, editor). Kept as an explicit empty array so future fields
// that genuinely need a restart can opt back in.
export const RESTART_REQUIRED_FIELDS: (keyof SettingsPayload)[] = [];
