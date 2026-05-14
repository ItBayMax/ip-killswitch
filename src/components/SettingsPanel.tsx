import { useStore } from "@/store";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import type { AppConfig } from "@/types";
import { UpdateChecker } from "@/components/UpdateChecker";

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
          <CardTitle>关于与更新</CardTitle>
          <CardDescription>
            当前应用版本与 GitHub Releases 的对比。启动后 6 小时内会静默检查一次；点按钮也可手动触发。
          </CardDescription>
        </CardHeader>
        <CardContent>
          <UpdateChecker />
        </CardContent>
      </Card>

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
