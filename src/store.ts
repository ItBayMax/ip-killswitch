import { create } from "zustand";
import { api } from "./api";
import type {
  AppConfig,
  DetectionReport,
  DiscoveredProcess,
  KillOutcome,
  SchedulerState,
} from "./types";

interface AppStore {
  config: AppConfig | null;
  report: DetectionReport | null;
  detecting: boolean;
  processes: DiscoveredProcess[];
  pendingKill: DetectionReport | null;
  logs: string;
  schedulerState: SchedulerState;
  elevated: boolean | null;
  lastKillOutcomes: KillOutcome[];
  setConfig: (cfg: AppConfig) => void;
  loadConfig: () => Promise<void>;
  saveConfig: (cfg: AppConfig) => Promise<void>;
  loadReport: () => Promise<void>;
  setReport: (r: DetectionReport | null) => void;
  setDetecting: (v: boolean) => void;
  detect: (override?: { providers?: AppConfig["providers"]; allowed?: string[] }) => Promise<void>;
  refreshProcesses: () => Promise<void>;
  killAllMatching: () => Promise<void>;
  killPids: (pids: number[]) => Promise<void>;
  setPendingKill: (r: DetectionReport | null) => void;
  loadLogs: () => Promise<void>;
  refreshSchedulerState: () => Promise<void>;
  pauseScheduler: () => Promise<void>;
  resumeScheduler: () => Promise<void>;
  refreshElevation: () => Promise<void>;
  dismissKillOutcomes: () => void;
}

export const useStore = create<AppStore>((set, get) => ({
  config: null,
  report: null,
  detecting: false,
  processes: [],
  pendingKill: null,
  logs: "",
  schedulerState: "disabled",
  elevated: null,
  lastKillOutcomes: [],
  setConfig: (cfg) => set({ config: cfg }),
  setReport: (r) => set({ report: r }),
  setDetecting: (v) => set({ detecting: v }),
  setPendingKill: (r) => set({ pendingKill: r }),
  loadConfig: async () => {
    const cfg = await api.getConfig();
    set({ config: cfg });
  },
  saveConfig: async (cfg) => {
    await api.saveConfig(cfg);
    set({ config: cfg });
    // saving may have changed the schedule; refresh scheduler indicator.
    try {
      const s = await api.schedulerStatus();
      set({ schedulerState: s });
    } catch {
      /* ignore */
    }
  },
  loadReport: async () => {
    const r = await api.lastReport();
    set({ report: r });
  },
  detect: async (override) => {
    set({ detecting: true });
    try {
      const options =
        override && (override.providers || override.allowed)
          ? { providers: override.providers, allowed_ips: override.allowed }
          : undefined;
      const r = await api.detectNow(options);
      set({ report: r });
    } finally {
      set({ detecting: false });
    }
  },
  refreshProcesses: async () => {
    const ps = await api.listTargetProcesses();
    set({ processes: ps });
  },
  killAllMatching: async () => {
    const outcomes = await api.killProcesses();
    set({ lastKillOutcomes: outcomes });
    await get().refreshProcesses();
  },
  killPids: async (pids) => {
    const outcomes = await api.killProcesses(pids);
    set({ lastKillOutcomes: outcomes });
    await get().refreshProcesses();
  },
  loadLogs: async () => {
    const text = await api.readLogs(256);
    set({ logs: text });
  },
  refreshSchedulerState: async () => {
    const s = await api.schedulerStatus();
    set({ schedulerState: s });
  },
  pauseScheduler: async () => {
    const s = await api.pauseScheduler();
    set({ schedulerState: s });
  },
  resumeScheduler: async () => {
    const s = await api.resumeScheduler();
    set({ schedulerState: s });
  },
  refreshElevation: async () => {
    try {
      const ok = await api.isElevated();
      set({ elevated: ok });
    } catch {
      set({ elevated: null });
    }
  },
  dismissKillOutcomes: () => set({ lastKillOutcomes: [] }),
}));
