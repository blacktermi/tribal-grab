import { useEffect, useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import { useQueue } from "@/stores/queue";

const toLocal = (unix: number | null | undefined) => {
  if (!unix) return "";
  const d = new Date(unix * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
};
const toUnix = (s: string) => (s ? Math.floor(new Date(s).getTime() / 1000) : null);

/** Date de début différée et, pour un direct, date d'arrêt. */
export function ScheduleDialog({ id, onClose }: { id: number | null; onClose: () => void }) {
  const task = useQueue((s) => (id != null ? s.tasks[id] : undefined));
  const [start, setStart] = useState("");
  const [stop, setStop] = useState("");
  useEffect(() => { setStart(toLocal(task?.scheduled_at)); setStop(toLocal(task?.stop_at)); }, [task?.id, task?.scheduled_at, task?.stop_at]);
  return (
    <Dialog open={id != null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogHeader><DialogTitle>Planifier</DialogTitle><DialogDescription>Laissez vide pour démarrer tout de suite.</DialogDescription></DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1"><Label>Début</Label><Input type="datetime-local" value={start} onChange={(e) => setStart(e.target.value)} className="num h-8 text-[12px]" /></div>
          <div className="space-y-1"><Label>Arrêt de l'enregistrement (direct)</Label><Input type="datetime-local" value={stop} onChange={(e) => setStop(e.target.value)} className="num h-8 text-[12px]" /></div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Annuler</Button>
          <Button onClick={async () => { if (id != null && (await attempt(ipc.setSchedule(id, toUnix(start), toUnix(stop)), "Planification enregistrée")) !== undefined) onClose(); }}>Enregistrer</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
