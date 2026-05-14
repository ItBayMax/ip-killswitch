import { useStore } from "@/store";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Plus, Trash2 } from "lucide-react";
import { shortId } from "@/lib/utils";
import type { Provider } from "@/types";

export function ProvidersPanel() {
  const { config, saveConfig } = useStore();
  if (!config) return null;

  function update(next: Partial<typeof config>) {
    if (!config) return;
    saveConfig({ ...config, ...next });
  }

  function setProviders(providers: Provider[]) {
    update({ providers });
  }

  function setAllowed(text: string) {
    update({ allowed_ips: text.split(/\r?\n|,/).map((s) => s.trim()).filter(Boolean) });
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>检测源 (HTTP 出口IP查询网站)</CardTitle>
          <CardDescription>
            程序按顺序请求每个启用的网站，自动解析 HTML 或 text/plain 响应中的 IPv4 / IPv6 地址。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <Label>命中策略：</Label>
              <select
                className="h-8 rounded-md border border-input bg-transparent px-2 text-sm"
                value={config.strategy}
                onChange={(e) =>
                  update({ strategy: e.target.value as "any" | "all" })
                }
              >
                <option value="any">任一返回即视为成功</option>
                <option value="all">全部返回且 IP 一致</option>
              </select>
            </div>
            <div className="flex items-center gap-2">
              <Label>失败重试次数：</Label>
              <Input
                type="number"
                min={1}
                max={20}
                value={config.retry}
                onChange={(e) => update({ retry: Math.max(1, Number(e.target.value || 1)) })}
                className="w-24"
              />
            </div>
            <div className="flex items-center gap-2">
              <Label>请求超时 (ms)：</Label>
              <Input
                type="number"
                min={500}
                max={60000}
                value={config.request_timeout_ms}
                onChange={(e) =>
                  update({ request_timeout_ms: Math.max(500, Number(e.target.value || 0)) })
                }
                className="w-28"
              />
            </div>
          </div>

          <div className="space-y-2">
            {config.providers.map((p, idx) => (
              <div
                key={p.id}
                className="grid grid-cols-12 gap-2 items-center border rounded-md p-2"
              >
                <div className="col-span-2">
                  <Input
                    placeholder="名称"
                    value={p.name}
                    onChange={(e) => {
                      const next = [...config.providers];
                      next[idx] = { ...p, name: e.target.value };
                      setProviders(next);
                    }}
                  />
                </div>
                <div className="col-span-5">
                  <Input
                    placeholder="https://api.ipify.org"
                    value={p.url}
                    onChange={(e) => {
                      const next = [...config.providers];
                      next[idx] = { ...p, url: e.target.value };
                      setProviders(next);
                    }}
                  />
                </div>
                <div className="col-span-3">
                  <Input
                    placeholder="可选自定义正则（默认自动解析）"
                    value={p.extract_regex ?? ""}
                    onChange={(e) => {
                      const next = [...config.providers];
                      next[idx] = { ...p, extract_regex: e.target.value || null };
                      setProviders(next);
                    }}
                  />
                </div>
                <div className="col-span-1 flex items-center gap-2">
                  <Switch
                    checked={p.enabled}
                    onCheckedChange={(v) => {
                      const next = [...config.providers];
                      next[idx] = { ...p, enabled: v };
                      setProviders(next);
                    }}
                  />
                </div>
                <div className="col-span-1 flex items-center justify-end">
                  <Button
                    size="icon"
                    variant="ghost"
                    onClick={() =>
                      setProviders(config.providers.filter((x) => x.id !== p.id))
                    }
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            ))}
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                setProviders([
                  ...config.providers,
                  {
                    id: shortId(),
                    name: `provider-${config.providers.length + 1}`,
                    url: "",
                    enabled: true,
                    extract_regex: null,
                  },
                ])
              }
            >
              <Plus className="h-4 w-4 mr-1" />
              新增检测源
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>目标出口IP白名单</CardTitle>
          <CardDescription>
            一行一个或逗号分隔；任何检测到的 IP 都会与此列表比对，不在列表内则触发告警与进程处理。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          <textarea
            className="w-full min-h-[120px] rounded-md border bg-transparent px-3 py-2 text-sm font-mono"
            value={config.allowed_ips.join("\n")}
            onChange={(e) => setAllowed(e.target.value)}
            placeholder="203.0.113.10&#10;198.51.100.5"
          />
          <div className="flex flex-wrap gap-2">
            {config.allowed_ips.map((ip) => (
              <Badge key={ip} variant="secondary">{ip}</Badge>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
