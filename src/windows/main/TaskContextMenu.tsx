import type { ReactNode } from "react";
import {
  ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuSub,
  ContextMenuSubContent, ContextMenuSubTrigger, ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import { canCancel, canPause, canResume, canRetry, useQueue } from "@/stores/queue";
import type { Priority, Task } from "@/types";
import { toast } from "sonner";

export function TaskContextMenu({ task, selectedIds, children, onDetails, onSchedule }: {
  task: Task; selectedIds: number[]; children: ReactNode; onDetails: (url: string) => void; onSchedule: (id: number) => void;
}) {
  const ids = selectedIds.includes(task.id) ? selectedIds : [task.id];
  const tasks = useQueue.getState().tasks;
  const many = ids.length > 1;
  const each = (f: (id: number) => Promise<unknown>) => Promise.all(ids.map((id) => attempt(f(id))));
  const prio = (p: Priority) => each((id) => ipc.setPriority(id, p));
  const move = (to: "first" | "last" | "up" | "down") => {
    const order = Object.values(tasks).sort((a, b) => a.position - b.position || a.id - b.id);
    const idx = order.findIndex((t) => t.id === task.id);
    const target = to === "first" ? 0 : to === "last" ? order.length - 1 : to === "up" ? Math.max(0, idx - 1) : Math.min(order.length - 1, idx + 1);
    return attempt(ipc.moveTask(task.id, target));
  };
  const finished = task.state === "Finished" && !!task.output_path;
  return (
    <ContextMenu onOpenChange={(o) => o && !selectedIds.includes(task.id) && useQueue.getState().select(task.id, "one")}>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-56">
        {finished && !many && (
          <>
            <ContextMenuItem onSelect={() => attempt(ipc.openPath(task.output_path!))}>Ouvrir</ContextMenuItem>
            <ContextMenuItem onSelect={() => attempt(ipc.quicklook(task.output_path!))}>Aperçu</ContextMenuItem>
            <ContextMenuItem onSelect={() => attempt(ipc.revealInFinder(task.output_path!))}>Afficher dans le Finder</ContextMenuItem>
            <ContextMenuSeparator />
          </>
        )}
        {canPause(task) && <ContextMenuItem onSelect={() => each(ipc.pause)}>Mettre en pause</ContextMenuItem>}
        {canResume(task) && <ContextMenuItem onSelect={() => each(ipc.resume)}>Reprendre</ContextMenuItem>}
        {canRetry(task) && <ContextMenuItem onSelect={() => each(ipc.retry)}>Relancer</ContextMenuItem>}
        {task.state === "Recording" && <ContextMenuItem onSelect={() => attempt(ipc.stopRecording(task.id))}>Arrêter l'enregistrement</ContextMenuItem>}
        {canCancel(task) && <ContextMenuItem onSelect={() => each(ipc.cancel)}>Annuler</ContextMenuItem>}
        <ContextMenuItem onSelect={() => each(ipc.remove)}>Retirer de la file</ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuSub>
          <ContextMenuSubTrigger>Priorité</ContextMenuSubTrigger>
          <ContextMenuSubContent>
            <ContextMenuItem onSelect={() => prio("High")}>Haute{task.priority === "High" ? " ✓" : ""}</ContextMenuItem>
            <ContextMenuItem onSelect={() => prio("Normal")}>Normale{task.priority === "Normal" ? " ✓" : ""}</ContextMenuItem>
            <ContextMenuItem onSelect={() => prio("Low")}>Basse{task.priority === "Low" ? " ✓" : ""}</ContextMenuItem>
          </ContextMenuSubContent>
        </ContextMenuSub>
        {!many && (
          <ContextMenuSub>
            <ContextMenuSubTrigger>Déplacer</ContextMenuSubTrigger>
            <ContextMenuSubContent>
              <ContextMenuItem onSelect={() => move("first")}>En première position</ContextMenuItem>
              <ContextMenuItem onSelect={() => move("up")}>Monter</ContextMenuItem>
              <ContextMenuItem onSelect={() => move("down")}>Descendre</ContextMenuItem>
              <ContextMenuItem onSelect={() => move("last")}>En dernière position</ContextMenuItem>
            </ContextMenuSubContent>
          </ContextMenuSub>
        )}
        {!many && (task.state === "Queued" || task.state === "Scheduled") && <ContextMenuItem onSelect={() => onSchedule(task.id)}>Planifier…</ContextMenuItem>}
        <ContextMenuSeparator />
        {!many && <ContextMenuItem onSelect={() => onDetails(task.url)}>Détails du lien…</ContextMenuItem>}
        <ContextMenuItem onSelect={() => { navigator.clipboard.writeText(ids.map((id) => tasks[id]?.url).filter(Boolean).join("\n")).then(() => toast.success("Lien copié")); }}>
          Copier le lien
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
