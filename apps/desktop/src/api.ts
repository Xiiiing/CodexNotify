import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, AppState, Diagnostics, EventRecord, EventStatus, HookStatus, RemoteDeleteResult, SecretStatus, StorageMode, TestBarkInput, TestBarkResult, TestHookResult } from "./types";

export const api = {
  state: () => invoke<AppState>("get_app_state"),
  selectStorage: (mode:StorageMode,customPath?:string) => invoke<void>("select_storage",{mode,customPath:customPath||null}),
  migrateStorage: (mode:StorageMode,customPath?:string) => invoke<void>("migrate_storage",{mode,customPath:customPath||null}),
  save: (settings:AppSettings) => invoke<void>("save_settings",{settings}),
  secretStatus: () => invoke<SecretStatus>("get_secret_status"),
  setSecret: (kind:string,value:string) => invoke<void>("set_secret",{kind,value}),
  deleteSecret: (kind:string) => invoke<void>("delete_secret",{kind}),
  testBark: (input:TestBarkInput) => invoke<TestBarkResult>("test_bark_connection",{input}),
  testHook: () => invoke<TestHookResult>("test_hook_delivery"),
  events: (status?:EventStatus) => invoke<EventRecord[]>("list_events",{limit:100,status:status||null}),
  retry: (id:number) => invoke<number>("retry_event",{id}),
  retryFailed: () => invoke<number>("retry_failed"),
  clearHistory: () => invoke<number>("clear_history"),
  updateRemote: (id:number,body:string) => invoke<void>("update_remote_notification",{id,body}),
  deleteRemote: (id:number) => invoke<void>("delete_remote_notification",{id}),
  deleteAllRemote: () => invoke<RemoteDeleteResult>("delete_all_remote_notifications"),
  hookStatus: () => invoke<HookStatus>("get_hook_status"),
  installHook: () => invoke<HookStatus>("install_hook"),
  uninstallHook: () => invoke<HookStatus>("uninstall_hook"),
  uninstallApplication: () => invoke<void>("uninstall_application"),
  diagnostics: () => invoke<Diagnostics>("run_diagnostics"),
  autostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled:boolean) => invoke<void>("set_autostart",{enabled}),
};
