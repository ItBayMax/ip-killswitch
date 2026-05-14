export type DetectStrategy = "any" | "all";
export type KillMode = "confirm" | "auto" | "manual";

export interface Provider {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  extract_regex?: string | null;
}

export interface ProcessTarget {
  id: string;
  label: string;
  name: string;
  enabled: boolean;
}

export type Schedule =
  | { kind: "disabled" }
  | { kind: "interval"; seconds: number }
  | { kind: "cron"; expr: string };

export interface AppConfig {
  providers: Provider[];
  allowed_ips: string[];
  processes: ProcessTarget[];
  strategy: DetectStrategy;
  kill_mode: KillMode;
  retry: number;
  request_timeout_ms: number;
  schedule: Schedule;
  autostart: boolean;
  minimize_to_tray: boolean;
  close_to_tray: boolean;
  confirm_exit: boolean;
  log_level: string;
}

export interface ProviderResult {
  provider_id: string;
  provider_name: string;
  url: string;
  ok: boolean;
  ip?: string | null;
  raw_excerpt?: string | null;
  status?: number | null;
  attempts: number;
  elapsed_ms: number;
  error?: string | null;
}

export interface DetectionReport {
  started_at: string;
  finished_at: string;
  strategy: DetectStrategy;
  providers: ProviderResult[];
  detected_ips: string[];
  matched: boolean;
  matched_ip?: string | null;
  allowed_ips: string[];
}

export interface DiscoveredProcess {
  pid: number;
  name: string;
  exe?: string | null;
  matched_target_id: string;
  matched_target_label: string;
}

export interface KillOutcome {
  pid: number;
  name: string;
  killed: boolean;
  error?: string | null;
}

export type SchedulerState = "disabled" | "paused" | "running";
