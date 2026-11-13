/**
 * Settings-related native calls, grouped for the settings feature. Every
 * call goes through `services/native.ts` (the only module that talks to
 * Rust); outside the desktop shell these reject with `unavailable`.
 */
export {
  clearActivityHistory,
  connectSettings,
  getActivitySnapshot,
  getAppInfo,
  getClaudeIntegration,
  getMediaSnapshot,
  getShortcutStatus,
  getSystemSnapshot,
  installClaudeIntegration,
  removeClaudeIntegration,
  setActivitySettings,
  testLocalReceiver,
  getDiagnostics,
  getStartupStatus,
  getStorageInfo,
  isNativeAvailable,
  listMonitors,
  openIslandyFolder,
  openSettingsWindow,
  previewNotificationSound,
  previewIslandOffset,
  resetSettings,
  setIslandVisible,
  setLaunchAtLogin,
  setToggleShortcut,
  updateSettings,
  searchWeatherPlaces,
  weatherLocate,
  getUpdateStatus,
  checkForUpdates,
  installUpdate,
} from "../../services/native";
