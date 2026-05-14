import { useEffect } from "react";
import { useStore } from "@/store";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { api } from "@/api";
import type { AppConfig } from "@/types";

export function LogsPanel() {
  const { config, saveConfig, logs, loadLogs } = useStore();

  useEffect(() => {
    loadLogs();
    const t = setInterval(loadLogs, 3000);
    return () => clearInterval(t);
  }, []);

  function updateLogLevel(level: string) {
    if (!config) return;
    const next: AppConfig = { ...config, log_level: level };
    saveConfig(next);
  }

  return (
    <Card>
      <CardHeader className="space-y-3">
        <div className="flex flex-row items-start justify-between gap-4">
          <div>
            <CardTitle>运行日志</CardTitle>
            <CardDescription>
              每 3 秒自动刷新；展示当日尾部 256KB。
            </CardDescription>
          </div>
          <div className="flex gap-2">
            <Button size="sm" variant="outline" onClick={loadLogs}>
              手动刷新
            </Button>
            <Button size="sm" variant="outline" onClick={() => api.openLogDir()}>
              打开日志目录
            </Button>
          </div>
        </div>
        <div className="flex items-center gap-2 text-sm">
          <Label>日志等级：</Label>
          <select
            className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
            value={config?.log_level ?? "info"}
            onChange={(e) => updateLogLevel(e.target.value)}
          >
            <option value="trace">trace</option>
            <option value="debug">debug</option>
            <option value="info">info</option>
            <option value="warn">warn</option>
            <option value="error">error</option>
          </select>
          <span className="text-xs text-muted-foreground">
            修改后下次启动生效。
          </span>
        </div>
      </CardHeader>
      <CardContent>
        <pre className="max-h-[55vh] overflow-auto rounded-md bg-muted p-3 text-xs leading-relaxed font-mono whitespace-pre-wrap">
{logs || "（暂无日志）"}
        </pre>
      </CardContent>
    </Card>
  );
}
