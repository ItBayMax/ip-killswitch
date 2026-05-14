import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export function ExitConfirmDialog({
  open,
  onClose,
  onConfirm,
  confirmExit,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  confirmExit: boolean;
}) {
  // If "confirm before exit" is disabled in settings, skip the dialog entirely.
  if (open && !confirmExit) {
    onConfirm();
    return null;
  }
  return (
    <Dialog open={open} onOpenChange={(v) => (!v ? onClose() : null)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>确认退出 IP Killswitch ?</DialogTitle>
          <DialogDescription>
            退出后将不再进行出口IP监测。可以选择最小化到托盘继续运行。
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            取消
          </Button>
          <Button variant="destructive" onClick={onConfirm}>
            退出
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
