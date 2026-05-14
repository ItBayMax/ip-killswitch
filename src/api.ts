import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  DetectionReport,
  DiscoveredProcess,
  KillOutcome,
  Provider,
  SchedulerState,
} from "./types";

export interface ManualOptions {
  providers?: Provider[];
  allowed_ips?: string[];
}

export const api = {
  getConfig: () => invoke<AppConfig>("get_config"),
  saveConfig: (cfg: AppConfig) => invoke<void>("save_config", { cfg }),
  detectNow: (options?: ManualOptions) =>
    invoke<DetectionReport>("detect_now", { options }),
  listTargetProcesses: () => invoke<DiscoveredProcess[]>("list_target_processes"),
  killProcesses: (pids?: number[]) =>
    invoke<KillOutcome[]>("kill_processes", { pids: pids ?? null }),
  lastReport: () => invoke<DetectionReport | null>("last_report"),
  readLogs: (maxKb?: number) => invoke<string>("read_logs", { maxKb }),
  openLogDir: () => invoke<void>("open_log_dir"),
  autostartStatus: () => invoke<boolean>("autostart_status"),
  setAutostart: (enabled: boolean) =>
    invoke<void>("set_autostart", { enabled }),
  quitApp: () => invoke<void>("quit_app"),
  showMainWindow: () => invoke<void>("show_main_window"),
  restartScheduler: () => invoke<void>("restart_scheduler"),
  schedulerStatus: () => invoke<SchedulerState>("scheduler_status"),
  pauseScheduler: () => invoke<SchedulerState>("pause_scheduler"),
  resumeScheduler: () => invoke<SchedulerState>("resume_scheduler"),
};
