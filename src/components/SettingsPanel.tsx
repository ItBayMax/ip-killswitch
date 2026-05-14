import { useStore } from "@/store";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { api } from "@/api";
import type { AppConfig } from "@/types";

export function SettingsPanel() {
  const { config, saveConfig } = useStore();
  if (!config) return null;
  const cfg: AppConfig = config;

  function update<K extends keyof AppConfig>(key: K, value: AppConfig[K]) {
    saveConfig({ ...cfg, [key]: value });
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>启动与托盘</CardTitle>
          <CardDescription>系统级行为</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Row
            label="开机/登录时自动启动"
            description="启用后会以 --minimized 参数静默运行，仅在托盘中显示。"
          >
            <Switch
              checked={cfg.autostart}
              onCheckedChange={(v) => update("autostart", v)}
            />
          </Row>
          <Row label="最小化到托盘" description="点击最小化按钮时隐藏到系统托盘。">
            <Switch
              checked={cfg.minimize_to_tray}
              onCheckedChange={(v) => update("minimize_to_tray", v)}
            />
          </Row>
          <Row label="关闭按钮等于最小化到托盘">
            <Switch
              checked={cfg.close_to_tray}
              onCheckedChange={(v) => update("close_to_tray", v)}
            />
          </Row>
          <Row label="退出前确认">
            <Switch
              checked={cfg.confirm_exit}
              onCheckedChange={(v) => update("confirm_exit", v)}
            />
          </Row>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>诊断</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center gap-2">
            <Label>日志等级：</Label>
            <select
              className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
              value={cfg.log_level}
              onChange={(e) => update("log_level", e.target.value)}
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
          <div className="flex gap-2">
            <Button size="sm" variant="outline" onClick={() => api.openLogDir()}>
              打开日志目录
            </Button>
            <Button size="sm" variant="outline" onClick={() => api.showMainWindow()}>
              测试激活窗口
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function Row({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div>
        <div className="text-sm font-medium">{label}</div>
        {description ? (
          <div className="text-xs text-muted-foreground">{description}</div>
        ) : null}
      </div>
      <div>{children}</div>
    </div>
  );
}
