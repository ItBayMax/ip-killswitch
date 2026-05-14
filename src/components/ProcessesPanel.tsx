import { useEffect } from "react";
import { useStore } from "@/store";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Plus, Trash2, RefreshCw, Skull, ShieldAlert, X } from "lucide-react";
import { shortId } from "@/lib/utils";
import { api } from "@/api";
import type { ProcessTarget } from "@/types";

export function ProcessesPanel() {
  const {
    config,
    saveConfig,
    processes,
    refreshProcesses,
    killAllMatching,
    killPids,
    elevated,
    lastKillOutcomes,
    dismissKillOutcomes,
  } = useStore();

  useEffect(() => {
    const t = setInterval(refreshProcesses, 5000);
    return () => clearInterval(t);
  }, []);

  if (!config) return null;

  function setTargets(processes: ProcessTarget[]) {
    if (!config) return;
    saveConfig({ ...config, processes });
  }

  async function restartAsAdmin() {
    try {
      const accepted = await api.relaunchAsAdmin();
      if (accepted) {
        // New elevated instance is launching; bow out so single-instance
        // doesn't collide.
        await api.quitApp();
      }
    } catch (e) {
      console.warn("relaunchAsAdmin failed:", e);
    }
  }

  const killFailures = lastKillOutcomes.filter((o) => !o.killed);

  return (
    <div className="space-y-4">
      {elevated === false ? (
        <Card className="border-amber-500/40 bg-amber-500/5">
          <CardContent className="flex items-center justify-between gap-4 py-3">
            <div className="flex items-start gap-2 text-sm">
              <ShieldAlert className="h-5 w-5 text-amber-600 mt-0.5 flex-shrink-0" />
              <div>
                <div className="font-medium">未以管理员身份运行</div>
                <div className="text-xs text-muted-foreground">
                  系统服务、其他用户拥有的进程将无法被结束，部分进程的可执行路径也读不到。
                </div>
              </div>
            </div>
            <Button size="sm" variant="default" onClick={restartAsAdmin}>
              以管理员身份重启
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {killFailures.length > 0 ? (
        <Card className="border-destructive/40 bg-destructive/5">
          <CardContent className="py-3">
            <div className="flex items-start justify-between gap-3">
              <div className="text-sm">
                <div className="font-medium text-destructive">
                  {killFailures.length} 个进程未能结束
                </div>
                <ul className="text-xs text-muted-foreground mt-1 space-y-0.5 font-mono max-h-32 overflow-auto">
                  {killFailures.map((o) => (
                    <li key={o.pid}>
                      [{o.pid}] {o.name || "(unknown)"} — {o.error ?? "unknown error"}
                    </li>
                  ))}
                </ul>
              </div>
              <Button size="icon" variant="ghost" onClick={dismissKillOutcomes}>
                <X className="h-4 w-4" />
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle>目标进程</CardTitle>
          <CardDescription>
            可填入 exe 名（如 <code>chrome.exe</code>）、进程名或可执行路径片段。
            匹配规则：完整等值优先，其次基于文件名等值，最后回退到子串匹配。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center gap-3">
            <Label>触发不匹配时的处理：</Label>
            <select
              className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
              value={config.kill_mode}
              onChange={(e) =>
                saveConfig({ ...config!, kill_mode: e.target.value as "auto" | "confirm" | "manual" })
              }
            >
              <option value="confirm">用户确认后 kill</option>
              <option value="auto">自动 kill（无需确认）</option>
              <option value="manual">仅通知，手动 kill</option>
            </select>
          </div>

          <div className="space-y-2">
            {config.processes.map((p, idx) => (
              <div key={p.id} className="grid grid-cols-12 gap-2 items-center border rounded-md p-2">
                <div className="col-span-4">
                  <Input
                    placeholder="标签 (UI 显示)"
                    value={p.label}
                    onChange={(e) => {
                      const next = [...config.processes];
                      next[idx] = { ...p, label: e.target.value };
                      setTargets(next);
                    }}
                  />
                </div>
                <div className="col-span-6">
                  <Input
                    placeholder="进程名 / exe 名 / 路径片段"
                    value={p.name}
                    onChange={(e) => {
                      const next = [...config.processes];
                      next[idx] = { ...p, name: e.target.value };
                      setTargets(next);
                    }}
                  />
                </div>
                <div className="col-span-1 flex items-center">
                  <Switch
                    checked={p.enabled}
                    onCheckedChange={(v) => {
                      const next = [...config.processes];
                      next[idx] = { ...p, enabled: v };
                      setTargets(next);
                    }}
                  />
                </div>
                <div className="col-span-1 flex items-center justify-end">
                  <Button
                    size="icon"
                    variant="ghost"
                    onClick={() => setTargets(config.processes.filter((x) => x.id !== p.id))}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            ))}
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  setTargets([
                    ...config.processes,
                    {
                      id: shortId(),
                      label: `进程 ${config.processes.length + 1}`,
                      name: "",
                      enabled: true,
                    },
                  ])
                }
              >
                <Plus className="h-4 w-4 mr-1" />
                新增进程
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <div>
            <CardTitle>当前匹配的运行中进程</CardTitle>
            <CardDescription>每 5 秒自动刷新；可手动结束。</CardDescription>
          </div>
          <div className="flex gap-2">
            <Button size="sm" variant="outline" onClick={refreshProcesses}>
              <RefreshCw className="h-4 w-4 mr-1" />
              刷新
            </Button>
            <Button size="sm" variant="destructive" onClick={killAllMatching} disabled={!processes.length}>
              <Skull className="h-4 w-4 mr-1" />
              结束全部
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {!processes.length ? (
            <div className="text-sm text-muted-foreground">未匹配到运行中的目标进程。</div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead className="text-left text-xs uppercase text-muted-foreground">
                  <tr>
                    <th className="py-2 pr-3">PID</th>
                    <th className="py-2 pr-3">名称</th>
                    <th className="py-2 pr-3">规则</th>
                    <th className="py-2 pr-3">路径</th>
                    <th className="py-2 pr-3 text-right">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {processes.map((p) => (
                    <tr key={p.pid} className="border-t">
                      <td className="py-2 pr-3 font-mono">{p.pid}</td>
                      <td className="py-2 pr-3 font-medium">{p.name}</td>
                      <td className="py-2 pr-3">
                        <Badge variant="secondary">{p.matched_target_label}</Badge>
                      </td>
                      <td className="py-2 pr-3 text-xs text-muted-foreground break-all">
                        {p.exe ?? "—"}
                      </td>
                      <td className="py-2 pr-3 text-right">
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => killPids([p.pid])}
                        >
                          结束
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
