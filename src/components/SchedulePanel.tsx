import { useStore } from "@/store";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Pause, Play, RotateCw } from "lucide-react";

const INTERVAL_OPTIONS: Array<{ label: string; seconds: number }> = [
  { label: "1 分钟", seconds: 60 },
  { label: "5 分钟", seconds: 300 },
  { label: "30 分钟", seconds: 1_800 },
  { label: "1 小时", seconds: 3_600 },
  { label: "2 小时", seconds: 7_200 },
  { label: "6 小时", seconds: 21_600 },
  { label: "12 小时", seconds: 43_200 },
  { label: "24 小时", seconds: 86_400 },
];

export function SchedulePanel() {
  const {
    config,
    saveConfig,
    schedulerState,
    pauseScheduler,
    resumeScheduler,
    refreshSchedulerState,
  } = useStore();
  if (!config) return null;
  const s = config.schedule;
  const kind = s.kind;

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4 space-y-0">
          <div>
            <CardTitle>定时自动检测</CardTitle>
            <CardDescription>
              可选择常用间隔，或填入 cron 表达式（6 字段：秒 分 时 日 月 周）。
            </CardDescription>
          </div>
          <RunStatus
            state={schedulerState}
            onPause={pauseScheduler}
            onResume={resumeScheduler}
            onRefresh={refreshSchedulerState}
          />
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center gap-3">
            <Label>模式：</Label>
            <select
              className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
              value={kind}
              onChange={(e) => {
                const v = e.target.value as "disabled" | "interval" | "cron";
                if (v === "disabled") {
                  saveConfig({ ...config, schedule: { kind: "disabled" } });
                } else if (v === "interval") {
                  saveConfig({ ...config, schedule: { kind: "interval", seconds: 300 } });
                } else {
                  saveConfig({ ...config, schedule: { kind: "cron", expr: "0 */5 * * * *" } });
                }
              }}
            >
              <option value="disabled">关闭</option>
              <option value="interval">固定间隔</option>
              <option value="cron">Cron 表达式</option>
            </select>
          </div>

          {kind === "interval" && (
            <div className="space-y-2">
              <Label>间隔</Label>
              <div className="flex flex-wrap gap-2">
                {INTERVAL_OPTIONS.map((o) => (
                  <Button
                    key={o.seconds}
                    size="sm"
                    variant={
                      s.kind === "interval" && s.seconds === o.seconds ? "default" : "outline"
                    }
                    onClick={() =>
                      saveConfig({
                        ...config,
                        schedule: { kind: "interval", seconds: o.seconds },
                      })
                    }
                  >
                    {o.label}
                  </Button>
                ))}
              </div>
              <div className="flex items-center gap-2 pt-2">
                <Label>自定义秒数：</Label>
                <Input
                  type="number"
                  min={30}
                  className="w-32"
                  value={s.kind === "interval" ? s.seconds : 300}
                  onChange={(e) =>
                    saveConfig({
                      ...config,
                      schedule: {
                        kind: "interval",
                        seconds: Math.max(30, Number(e.target.value || 300)),
                      },
                    })
                  }
                />
                <span className="text-xs text-muted-foreground">最小 30 秒</span>
              </div>
            </div>
          )}

          {kind === "cron" && (
            <div className="space-y-2">
              <Label>Cron 表达式 (秒 分 时 日 月 周)</Label>
              <Input
                value={s.kind === "cron" ? s.expr : ""}
                onChange={(e) =>
                  saveConfig({ ...config, schedule: { kind: "cron", expr: e.target.value } })
                }
                placeholder="0 */5 * * * *"
                className="font-mono"
              />
              <div className="text-xs text-muted-foreground">
                例：<code>0 */5 * * * *</code> — 每 5 分钟；<code>0 0 9 * * *</code> — 每天 9 点。
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function RunStatus({
  state,
  onPause,
  onResume,
  onRefresh,
}: {
  state: "disabled" | "paused" | "running";
  onPause: () => void;
  onResume: () => void;
  onRefresh: () => void;
}) {
  let badge: React.ReactNode;
  let action: React.ReactNode;
  if (state === "running") {
    badge = <Badge variant="success">运行中</Badge>;
    action = (
      <Button size="sm" variant="outline" onClick={onPause}>
        <Pause className="h-4 w-4 mr-1" />
        暂停检测
      </Button>
    );
  } else if (state === "paused") {
    badge = <Badge variant="warning">已暂停</Badge>;
    action = (
      <Button size="sm" onClick={onResume}>
        <Play className="h-4 w-4 mr-1" />
        恢复检测
      </Button>
    );
  } else {
    badge = <Badge variant="outline">未启用</Badge>;
    action = null;
  }
  return (
    <div className="flex items-center gap-2">
      {badge}
      {action}
      <Button size="icon" variant="ghost" onClick={onRefresh} title="刷新状态">
        <RotateCw className="h-4 w-4" />
      </Button>
    </div>
  );
}
