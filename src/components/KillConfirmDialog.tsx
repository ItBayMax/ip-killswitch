import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import type { DetectionReport } from "@/types";
import { useStore } from "@/store";
import { useEffect } from "react";

export function KillConfirmDialog({
  open,
  report,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  report: DetectionReport | null;
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
}) {
  const { processes, refreshProcesses } = useStore();
  useEffect(() => {
    if (open) refreshProcesses();
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={(v) => (!v ? onCancel() : null)}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>⚠ 出口IP不匹配</DialogTitle>
          <DialogDescription>
            检测到的IP不在允许列表内，是否立刻结束已匹配的目标进程？
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3 text-sm">
          <div>
            <div className="text-xs text-muted-foreground mb-1">检测到</div>
            <div className="flex flex-wrap gap-2">
              {(report?.detected_ips ?? []).map((ip) => (
                <Badge key={ip} variant="destructive">{ip}</Badge>
              ))}
              {(!report?.detected_ips || report.detected_ips.length === 0) && (
                <span className="text-muted-foreground">未获取到 IP</span>
              )}
            </div>
          </div>
          <div>
            <div className="text-xs text-muted-foreground mb-1">允许列表</div>
            <div className="flex flex-wrap gap-2">
              {(report?.allowed_ips ?? []).map((ip) => (
                <Badge key={ip} variant="secondary">{ip}</Badge>
              ))}
            </div>
          </div>
          <div>
            <div className="text-xs text-muted-foreground mb-1">
              将被结束的进程 ({processes.length})
            </div>
            <ul className="text-xs font-mono max-h-40 overflow-auto border rounded p-2 bg-muted">
              {processes.length ? (
                processes.map((p) => (
                  <li key={p.pid}>
                    [{p.pid}] {p.name} — {p.matched_target_label}
                  </li>
                ))
              ) : (
                <li className="text-muted-foreground">无</li>
              )}
            </ul>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>
            稍后
          </Button>
          <Button variant="destructive" onClick={() => onConfirm()}>
            立即结束 ({processes.length})
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
