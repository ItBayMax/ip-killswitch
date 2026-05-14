import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { Shield, ShieldAlert, Activity, Settings, Cpu, Clock, FileText } from "lucide-react";
import { useStore } from "./store";
import { api } from "./api";
import type { DetectionReport } from "./types";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Dashboard } from "./components/Dashboard";
import { ProvidersPanel } from "./components/ProvidersPanel";
import { ProcessesPanel } from "./components/ProcessesPanel";
import { SchedulePanel } from "./components/SchedulePanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { LogsPanel } from "./components/LogsPanel";
import { KillConfirmDialog } from "./components/KillConfirmDialog";
import { ExitConfirmDialog } from "./components/ExitConfirmDialog";

export default function App() {
  const {
    config,
    report,
    detecting,
    loadConfig,
    loadReport,
    setReport,
    setDetecting,
    setPendingKill,
    pendingKill,
    refreshProcesses,
    killAllMatching,
    refreshSchedulerState,
  } = useStore();
  const [tab, setTab] = useState("dashboard");
  const [exitOpen, setExitOpen] = useState(false);

  useEffect(() => {
    loadConfig();
    loadReport();
    refreshProcesses();
    refreshSchedulerState();
  }, []);

  useEffect(() => {
    const unsubs: Array<() => void> = [];
    (async () => {
      unsubs.push(
        await listen("ipkillswitch://detection-started", () => setDetecting(true))
      );
      unsubs.push(
        await listen<DetectionReport>("ipkillswitch://detection-finished", (e) => {
          setDetecting(false);
          setReport(e.payload);
        })
      );
      unsubs.push(
        await listen<DetectionReport>("ipkillswitch://prompt-kill", (e) => {
          setPendingKill(e.payload);
        })
      );
      unsubs.push(
        await listen("ipkillswitch://request-exit", () => setExitOpen(true))
      );
      unsubs.push(
        await listen("ipkillswitch://scheduler-changed", () => refreshSchedulerState())
      );
    })();
    return () => unsubs.forEach((u) => u());
  }, []);

  const matched = report?.matched ?? null;
  const matchedIp = report?.matched_ip ?? null;
  const detected = report?.detected_ips ?? [];

  return (
    <div className="min-h-screen flex flex-col">
      <header className="border-b px-6 py-3 flex items-center justify-between bg-card">
        <div className="flex items-center gap-3">
          {matched === false ? (
            <ShieldAlert className="h-6 w-6 text-destructive" />
          ) : (
            <Shield className="h-6 w-6 text-primary" />
          )}
          <div>
            <div className="font-semibold leading-tight">IP Killswitch</div>
            <div className="text-xs text-muted-foreground">出口IP监测与目标进程管控</div>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <Status report={report} detecting={detecting} />
          <Button
            size="sm"
            variant={matched === false ? "destructive" : "default"}
            onClick={() => useStore.getState().detect()}
            disabled={detecting}
          >
            {detecting ? "检测中…" : "立即检测"}
          </Button>
        </div>
      </header>

      <main className="flex-1 px-6 py-4 overflow-auto">
        <Tabs value={tab} onValueChange={setTab} className="w-full">
          <TabsList>
            <TabsTrigger value="dashboard"><Activity className="h-4 w-4 mr-1" />仪表盘</TabsTrigger>
            <TabsTrigger value="providers"><Shield className="h-4 w-4 mr-1" />检测源 / 目标IP</TabsTrigger>
            <TabsTrigger value="processes"><Cpu className="h-4 w-4 mr-1" />进程</TabsTrigger>
            <TabsTrigger value="schedule"><Clock className="h-4 w-4 mr-1" />定时</TabsTrigger>
            <TabsTrigger value="settings"><Settings className="h-4 w-4 mr-1" />设置</TabsTrigger>
            <TabsTrigger value="logs"><FileText className="h-4 w-4 mr-1" />日志</TabsTrigger>
          </TabsList>
          <TabsContent value="dashboard"><Dashboard /></TabsContent>
          <TabsContent value="providers"><ProvidersPanel /></TabsContent>
          <TabsContent value="processes"><ProcessesPanel /></TabsContent>
          <TabsContent value="schedule"><SchedulePanel /></TabsContent>
          <TabsContent value="settings"><SettingsPanel /></TabsContent>
          <TabsContent value="logs"><LogsPanel /></TabsContent>
        </Tabs>
      </main>

      <KillConfirmDialog
        open={pendingKill !== null}
        report={pendingKill}
        onConfirm={async () => {
          setPendingKill(null);
          await killAllMatching();
        }}
        onCancel={() => setPendingKill(null)}
      />

      <ExitConfirmDialog
        open={exitOpen}
        onClose={() => setExitOpen(false)}
        onConfirm={async () => {
          setExitOpen(false);
          await api.quitApp();
        }}
        confirmExit={config?.confirm_exit ?? true}
      />
    </div>
  );
}

function Status({
  report,
  detecting,
}: {
  report: DetectionReport | null;
  detecting: boolean;
}) {
  if (detecting) {
    return <Badge variant="secondary">检测中…</Badge>;
  }
  if (!report) return <Badge variant="outline">未检测</Badge>;
  if (report.allowed_ips.length === 0) {
    return (
      <Badge variant="warning">
        未配置目标IP · 检测到 {report.detected_ips.join(", ") || "—"}
      </Badge>
    );
  }
  if (report.matched) {
    return (
      <Badge variant="success">匹配 · {report.matched_ip}</Badge>
    );
  }
  return (
    <Badge variant="destructive">
      不匹配 · 检测到 {report.detected_ips.join(", ") || "—"}
    </Badge>
  );
}
