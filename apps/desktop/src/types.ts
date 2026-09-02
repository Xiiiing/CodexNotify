export type EventStatus =
  "queued" | "sending" | "retrying" | "sent" | "failed" | "suppressed";
export interface ProjectRule {
  path: string;
  name: string;
  enabled: boolean;
}
export interface AppSettings {
  schemaVersion: number;
  enabled: boolean;
  barkServer: string;
  group: string;
  level: string;
  sound: string;
  scope: string;
  projects: ProjectRule[];
  messageMode: string;
  fixedMessage: string;
  notificationTitle: string;
  permissionNotifications: boolean;
  redactSensitive: boolean;
  quietHoursEnabled: boolean;
  quietStart: string;
  quietEnd: string;
  quietAction: string;
  barkIcon: string;
  clickUrl: string;
  requestTimeout: number;
  retryLimit: number;
  encryptionEnabled: boolean;
  encryptionAlgorithm: string;
  setupCompleted: boolean;
  language: string;
  theme: string;
}
export interface EventCounts {
  queued: number;
  sending: number;
  retrying: number;
  sent: number;
  failed: number;
  suppressed: number;
}
export type HookTrustStatus =
  "notInstalled" | "trusted" | "untrusted" | "modified" | "unknown";
export interface HookStatus {
  hooksPath: string;
  exists: boolean;
  installed: boolean;
  handlerCount: number;
  installedEvents: string[];
  pathCurrent: boolean;
  configuredCommand: string;
  trusted: boolean;
  trustStatus: HookTrustStatus;
  reviewRequired: boolean;
  enabled: boolean;
}
export interface SecretStatus {
  barkKeyConfigured: boolean;
  encryptionKeyConfigured: boolean;
}
export type StorageMode = "default" | "portable" | "custom" | "environment";
export interface StorageInfo {
  configured: boolean;
  mode: StorageMode;
  root: string;
  configDir: string;
  dataDir: string;
  logDir: string;
  locatorFile: string;
}
export interface HookHealth {
  status?: "success" | "error";
  stage?: string;
  eventType?: string;
  deliveryStatus?: EventStatus | "filtered" | "disabled" | "ignored" | "unknown";
  lastAttemptAt?: string;
  lastSuccessAt?: string;
  errorCode?: string;
  message?: string;
  project?: string;
}
export interface AppState {
  storage: StorageInfo;
  settings: AppSettings;
  counts: EventCounts;
  secrets: SecretStatus;
  hook: HookStatus;
  health: HookHealth;
}
export interface EventRecord {
  id: number;
  eventKey: string;
  eventType: string;
  project: string;
  title: string;
  subtitle: string;
  body: string;
  status: EventStatus;
  attempts: number;
  nextAttemptAt: number;
  createdAt: number;
  sentAt?: number;
  error: string;
}
export interface Diagnostics {
  storage: StorageInfo;
  settingsReadable: boolean;
  databaseReady: boolean;
  credentialStoreAvailable: boolean;
  hook: HookStatus;
  hookBinary: string;
  hookBinaryExists: boolean;
  health: HookHealth;
}
export interface ApiError {
  code: string;
  message: string;
}
export interface TestBarkInput {
  settings: AppSettings;
  barkKey?: string;
  encryptionKey?: string;
}
export interface TestBarkResult {
  ok: true;
  elapsedMs: number;
}
export interface TestHookResult {
  ok: boolean;
  elapsedMs: number;
  deliveryStatus: EventStatus | "unknown";
  errorCode: string;
  message: string;
}
