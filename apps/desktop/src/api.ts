import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, AppState, Diagnostics, EventRecord, EventStatus, HookStatus, SecretStatus, StorageMode, TestBarkInput, TestBarkResult } from "./types";

export const api = {
  state: () => invoke<AppState>("get_app_state"),
  selectStorage: (mode:StorageMode,customPath?:string) => invoke<void>("select_storage",{mode,customPath:customPath||null}),
  migrateStorage: (mode:StorageMode,customPath?:string) => invoke<void>("migrate_storage",{mode,customPath:customPath||null}),
  save: (settings:AppSettings) => invoke<void>("save_settings",{settings}),
  secretStatus: () => invoke<SecretStatus>("get_secret_status"),
  setSecret: (kind:string,value:string) => invoke<void>("set_secret",{kind,value}),
  deleteSecret: (kind:string) => invoke<void>("delete_secret",{kind}),
  testBark: (input:TestBarkInput) => invoke<TestBarkResult>("test_bark_connection",{input}),
  events: (status?:EventStatus) => invoke<EventRecord[]>("list_events",{limit:100,status:status||null}),
  retry: (id:number) => invoke<number>("retry_event",{id}),
  retryFailed: () => invoke<number>("retry_failed"),
  clearHistory: () => invoke<number>("clear_history"),
  hookStatus: () => invoke<HookStatus>("get_hook_status"),
  installHook: () => invoke<HookStatus>("install_hook"),
  uninstallHook: () => invoke<HookStatus>("uninstall_hook"),
  diagnostics: () => invoke<Diagnostics>("run_diagnostics"),
  autostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled:boolean) => invoke<void>("set_autostart",{enabled}),
};
