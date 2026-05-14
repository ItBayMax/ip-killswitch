import { useState } from "react";
import { useStore } from "@/store";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { formatTime } from "@/lib/utils";
import { CheckCircle2, XCircle, AlertTriangle, Play } from "lucide-react";

export function Dashboard() {
  const { report, detecting, detect, config } = useStore();

  const [oneShotUrl, setOneShotUrl] = useState("");
  const [oneShotAllowed, setOneShotAllowed] = useState("");

  async function runOneShot() {
    const providers = oneShotUrl.trim()
      ? oneShotUrl
          .split(/\r?\n|,/)
          .map((u) => u.trim())
          .filter(Boolean)
          .map((url, i) => ({
            id: `manual-${i}`,
            name: `manual-${i + 1}`,
            url,
            enabled: true,
            extract_regex: null,
          }))
      : undefined;
    const allowed = oneShotAllowed.trim()
      ? oneShotAllowed
          .split(/\r?\n|,/)
          .map((s) => s.trim())
          .filter(Boolean)
      : undefined;
    await detect({ providers, allowed });
  }

  return (
    <div className="space-y-4">
      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>当前状态</CardTitle>
            <CardDescription>
              {report
                ? `上次检测 ${formatTime(report.finished_at)} · 耗时 ${
                    new Date(report.finished_at).getTime() -
                    new Date(report.started_at).getTime()
                  }ms`
                : "尚未运行检测"}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center gap-2">
              {report?.matched ? (
                <CheckCircle2 className="h-5 w-5 text-emerald-500" />
              ) : report?.allowed_ips.length === 0 ? (
                <AlertTriangle className="h-5 w-5 text-amber-500" />
              ) : (
                <XCircle className="h-5 w-5 text-destructive" />
              )}
              <div>
                <div className="text-sm font-medium">
                  {report?.matched
                    ? `匹配预配置 ${report.matched_ip}`
                    : report?.allowed_ips.length === 0
                    ? "未配置目标IP，仅展示检测结果"
                    : "出口IP与目标不匹配"}
                </div>
                <div className="text-xs text-muted-foreground">
                  策略：{config?.strategy === "all" ? "全部命中" : "任一命中"} · 重试 {config?.retry ?? 0} 次 · 超时 {config?.request_timeout_ms ?? 0}ms
                </div>
              </div>
            </div>

            <div>
              <div className="text-xs text-muted-foreground mb-1">检测到的IP</div>
              <div className="flex flex-wrap gap-2">
                {report?.detected_ips.length ? (
                  report.detected_ips.map((ip) => (
                    <Badge
                      key={ip}
                      variant={report.allowed_ips.includes(ip) ? "success" : "outline"}
                    >
                      {ip}
                      {report.allowed_ips.includes(ip) ? " ✓" : ""}
                    </Badge>
                  ))
                ) : (
                  <span className="text-sm text-muted-foreground">—</span>
                )}
              </div>
            </div>

            <div>
              <div className="text-xs text-muted-foreground mb-1">允许的目标IP</div>
              <div className="flex flex-wrap gap-2">
                {config?.allowed_ips.length ? (
                  config.allowed_ips.map((ip) => <Badge key={ip} variant="secondary">{ip}</Badge>)
                ) : (
                  <span className="text-sm text-muted-foreground">未配置</span>
                )}
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>手动一次性检测</CardTitle>
            <CardDescription>
              直接填入 URL 与目标 IP，不会改动持久化配置；为空时使用当前配置。
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="space-y-1">
              <Label>检测网站 (一行一个，或逗号分隔)</Label>
              <Input
                placeholder="https://api.ipify.org, https://ifconfig.me/ip"
                value={oneShotUrl}
                onChange={(e) => setOneShotUrl(e.target.value)}
              />
            </div>
            <div className="space-y-1">
              <Label>期望的出口 IP</Label>
              <Input
                placeholder="203.0.113.10, 198.51.100.5"
                value={oneShotAllowed}
                onChange={(e) => setOneShotAllowed(e.target.value)}
              />
            </div>
            <Button onClick={runOneShot} disabled={detecting}>
              <Play className="h-4 w-4 mr-1" />
              执行
            </Button>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>各检测源结果</CardTitle>
          <CardDescription>每个网站的返回与解析详情</CardDescription>
        </CardHeader>
        <CardContent>
          {!report?.providers?.length ? (
            <div className="text-sm text-muted-foreground">暂无</div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead className="text-left text-xs uppercase text-muted-foreground">
                  <tr>
                    <th className="py-2 pr-3">名称</th>
                    <th className="py-2 pr-3">URL</th>
                    <th className="py-2 pr-3">状态</th>
                    <th className="py-2 pr-3">解析出 IP</th>
                    <th className="py-2 pr-3">尝试</th>
                    <th className="py-2 pr-3">耗时</th>
                    <th className="py-2 pr-3">片段 / 错误</th>
                  </tr>
                </thead>
                <tbody>
                  {report.providers.map((p) => (
                    <tr key={p.provider_id} className="border-t">
                      <td className="py-2 pr-3 font-medium">{p.provider_name}</td>
                      <td className="py-2 pr-3 text-muted-foreground break-all">{p.url}</td>
                      <td className="py-2 pr-3">
                        {p.ok ? (
                          <Badge variant="success">HTTP {p.status ?? "?"}</Badge>
                        ) : (
                          <Badge variant="destructive">{p.status ?? "ERR"}</Badge>
                        )}
                      </td>
                      <td className="py-2 pr-3 font-mono">{p.ip ?? "—"}</td>
                      <td className="py-2 pr-3">{p.attempts}</td>
                      <td className="py-2 pr-3">{p.elapsed_ms} ms</td>
                      <td className="py-2 pr-3 text-xs text-muted-foreground break-all">
                        {p.error ?? p.raw_excerpt ?? ""}
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
