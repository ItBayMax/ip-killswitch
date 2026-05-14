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

  const refreshSec = config?.process_refresh_seconds ?? 5;
  useEffect(() => {
    refreshProcesses();
    if (refreshSec <= 0) return; // manual-only
    const t = setInterval(refreshProcesses, refreshSec * 1000);
    return () => clearInterval(t);
  }, [refreshSec]);

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
            可填入 exe 名（如 <code>claude.exe</code>）或进程名关键字。
            匹配规则：完整等值 → 文件名等值 → 进程名子串。
            每行可单独配置「忽略大小写 / 匹配可执行路径 / 关联子进程 / 启动即拦截」。
            路径匹配 + 启动即拦截需要先开管理员模式才能稳定工作。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
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
            <label
              className="flex items-center gap-2 text-xs text-muted-foreground cursor-pointer"
              title="防火墙规则除了阻断当前运行中的匹配进程，也阻断本次会话内任何曾经匹配过的 exe 路径。可防御「kill 完用户立刻重启」的循环，但会同时阻断同路径下的其他正常用途。"
            >
              <Switch
                checked={config.firewall_block_include_historical_paths}
                onCheckedChange={(v) =>
                  saveConfig({ ...config!, firewall_block_include_historical_paths: v })
                }
              />
              <span>阻断历史发现的路径</span>
            </label>
          </div>

          <div className="space-y-2">
            {config.processes.map((p, idx) => (
              <div key={p.id} className="border rounded-md p-2 space-y-2">
                <div className="grid grid-cols-12 gap-2 items-center">
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
                <div className="flex flex-wrap items-center gap-x-4 gap-y-2 pl-1 text-xs text-muted-foreground">
                  <label className="flex items-center gap-2 cursor-pointer">
                    <Switch
                      checked={p.case_insensitive}
                      onCheckedChange={(v) => {
                        const next = [...config.processes];
                        next[idx] = { ...p, case_insensitive: v };
                        setTargets(next);
                      }}
                    />
                    <span>忽略大小写</span>
                  </label>
                  <label
                    className="flex items-center gap-2 cursor-pointer"
                    title="把关键字作为完整 exe 路径的子串来匹配。短关键词易误伤，请下方表格里核对结果。"
                  >
                    <Switch
                      checked={p.match_path}
                      onCheckedChange={(v) => {
                        const next = [...config.processes];
                        next[idx] = { ...p, match_path: v };
                        setTargets(next);
                      }}
                    />
                    <span>匹配可执行路径</span>
                  </label>
                  <label className="flex items-center gap-2 cursor-pointer">
                    <Switch
                      checked={p.match_children}
                      onCheckedChange={(v) => {
                        const next = [...config.processes];
                        next[idx] = { ...p, match_children: v };
                        setTargets(next);
                      }}
                    />
                    <span>关联子进程（树遍历）</span>
                  </label>
                  <label
                    className="flex items-center gap-2 cursor-pointer"
                    title="进程一启动就检查 IP 并按需结束（事件驱动，毫秒级响应）。仅 Windows，需要管理员运行。建议给确实敏感的进程开。"
                  >
                    <Switch
                      checked={p.intercept_on_launch}
                      onCheckedChange={(v) => {
                        const next = [...config.processes];
                        next[idx] = { ...p, intercept_on_launch: v };
                        setTargets(next);
                      }}
                    />
                    <span>启动即拦截</span>
                  </label>
                  <label
                    className="flex items-center gap-2 cursor-pointer"
                    title="IP 不匹配时，对该进程的 exe 路径加 netsh 出站拦截规则；IP 恢复时自动撤销。仅 Windows，需要管理员；规则在 wf.msc 中以 ip-killswitch: 开头。"
                  >
                    <Switch
                      checked={p.firewall_block}
                      onCheckedChange={(v) => {
                        const next = [...config.processes];
                        next[idx] = { ...p, firewall_block: v };
                        setTargets(next);
                      }}
                    />
                    <span>阻断出站网络</span>
                  </label>
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
                      case_insensitive: true,
                      match_children: false,
                      match_path: false,
                      intercept_on_launch: false,
                      firewall_block: false,
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
            <CardDescription>
              {refreshSec > 0
                ? `每 ${refreshSec} 秒自动刷新；可手动结束。`
                : "自动刷新已关闭，使用右侧按钮手动刷新。"}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-2 text-xs text-muted-foreground">
              <span>刷新间隔</span>
              <select
                className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
                value={refreshSec}
                onChange={(e) =>
                  saveConfig({
                    ...config!,
                    process_refresh_seconds: Number(e.target.value),
                  })
                }
              >
                <option value={5}>5 秒</option>
                <option value={10}>10 秒</option>
                <option value={30}>30 秒</option>
                <option value={60}>1 分钟</option>
                <option value={300}>5 分钟</option>
                <option value={0}>关闭</option>
              </select>
            </label>
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
                      <td className="py-2 pr-3 space-x-1">
                        <Badge variant="secondary">{p.matched_target_label}</Badge>
                        {p.via_children ? (
                          <Badge variant="outline" className="text-xs">子</Badge>
                        ) : null}
                        {p.via_path ? (
                          <Badge variant="outline" className="text-xs" title="经由可执行路径匹配">路径</Badge>
                        ) : null}
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
