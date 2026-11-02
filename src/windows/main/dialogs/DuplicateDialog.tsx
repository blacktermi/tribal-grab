import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Thumb } from "@/components/app/Thumb";
import { dateTime } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import type { HistoryItem, Task } from "@/types";

export function DuplicateDialog({ task, previous }: { task: Task; previous: HistoryItem }) {
  return (
    <Dialog open>
      <DialogContent className="max-w-md" showCloseButton={false} onEscapeKeyDown={(e) => e.preventDefault()} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>Déjà téléchargé</DialogTitle>
          <DialogDescription>Cette vidéo figure déjà dans votre historique.</DialogDescription>
        </DialogHeader>
        <div className="flex gap-3 rounded-md border border-border bg-card p-3">
          <Thumb src={previous.thumbnail} className="h-[44px] w-[78px]" />
          <div className="min-w-0 text-[12.5px]">
            <div className="selectable truncate font-medium">{previous.title}</div>
            <div className="text-muted-foreground">{previous.site} · {dateTime(previous.finished_at)}</div>
            <div className="text-muted-foreground">{previous.exists ? "Le fichier est toujours sur le disque." : "Le fichier n'existe plus."}</div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => attempt(ipc.answer(task.id, { Duplicate: { action: "Skip" } }))}>Ignorer</Button>
          <Button onClick={() => attempt(ipc.answer(task.id, { Duplicate: { action: "Redownload" } }))}>Retélécharger</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
