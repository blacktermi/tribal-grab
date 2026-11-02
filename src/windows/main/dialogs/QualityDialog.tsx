import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { bytes, taskTitle } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import type { FormatChoice, Task } from "@/types";

export function QualityDialog({ task, choices }: { task: Task; choices: FormatChoice[] }) {
  const firstVideo = choices.find((c) => !c.audio_only) ?? choices[0];
  const [sel, setSel] = useState(firstVideo?.id ?? "");
  const video = choices.filter((c) => !c.audio_only);
  const audio = choices.filter((c) => c.audio_only);
  const Group = ({ title, items }: { title: string; items: FormatChoice[] }) =>
    items.length === 0 ? null : (
      <div>
        <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">{title}</div>
        <div className="rounded-md border border-border">
          {items.map((c) => (
            <label key={c.id} className="flex cursor-default items-center gap-3 border-b border-border/60 px-3 py-2 last:border-b-0 hover:bg-row-hover">
              <RadioGroupItem value={c.id} />
              <span className="flex-1 text-[13px]">{c.label}</span>
              <span className="num w-12 text-[12px] uppercase text-muted-foreground">{c.ext}</span>
              <span className="num w-20 text-right text-[12px] text-muted-foreground">{bytes(c.size)}</span>
            </label>
          ))}
        </div>
      </div>
    );
  return (
    <Dialog open>
      <DialogContent className="max-w-md" showCloseButton={false} onEscapeKeyDown={(e) => e.preventDefault()} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>Choisir la qualité</DialogTitle>
          <DialogDescription className="selectable truncate">{taskTitle(task)}</DialogDescription>
        </DialogHeader>
        <RadioGroup value={sel} onValueChange={setSel} className="max-h-[52vh] gap-3 overflow-y-auto">
          <Group title="Vidéo" items={video} />
          <Group title="Audio seul" items={audio} />
        </RadioGroup>
        <DialogFooter>
          <Button variant="ghost" onClick={() => attempt(ipc.remove(task.id))}>Annuler le téléchargement</Button>
          <Button disabled={!sel} onClick={() => attempt(ipc.answer(task.id, { Quality: { format_id: sel } }))}>Télécharger</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
