import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { taskTitle } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import type { Task } from "@/types";

export function DestinationDialog({ task, suggested }: { task: Task; suggested: string }) {
  const [dir, setDir] = useState(suggested);
  async function choose() {
    const r = await open({ directory: true, defaultPath: dir || undefined, title: "Dossier de destination" });
    if (typeof r === "string") setDir(r);
  }
  return (
    <Dialog open>
      <DialogContent className="max-w-md" showCloseButton={false} onEscapeKeyDown={(e) => e.preventDefault()} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>Choisir la destination</DialogTitle>
          <DialogDescription className="selectable truncate">{taskTitle(task)}</DialogDescription>
        </DialogHeader>
        <div className="flex gap-2">
          <Input value={dir} onChange={(e) => setDir(e.target.value)} className="num h-8 text-[12px]" spellCheck={false} />
          <Button variant="outline" size="sm" onClick={choose}>Choisir…</Button>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => attempt(ipc.remove(task.id))}>Annuler le téléchargement</Button>
          <Button disabled={!dir.trim()} onClick={() => attempt(ipc.answer(task.id, { Destination: { dir: dir.trim() } }))}>Télécharger ici</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
