import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { taskTitle } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import type { SubtitleTrack, Task } from "@/types";

export function SubtitlesDialog({ task, tracks }: { task: Task; tracks: SubtitleTrack[] }) {
  const langs = [...new Map(tracks.map((t) => [t.lang, t])).values()];
  const [sel, setSel] = useState<string[]>(() => langs.filter((l) => /^(fr|en)/i.test(l.lang) && !l.auto).slice(0, 2).map((l) => l.lang));
  const toggle = (l: string) => setSel((s) => (s.includes(l) ? s.filter((x) => x !== l) : [...s, l]));
  return (
    <Dialog open>
      <DialogContent className="max-w-md" showCloseButton={false} onEscapeKeyDown={(e) => e.preventDefault()} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>Sous-titres</DialogTitle>
          <DialogDescription className="selectable truncate">{taskTitle(task)}</DialogDescription>
        </DialogHeader>
        <div className="flex gap-2 text-[12px]">
          <button className="text-primary hover:underline" onClick={() => setSel(langs.map((l) => l.lang))}>Tout</button>
          <button className="text-primary hover:underline" onClick={() => setSel([])}>Aucun</button>
          <span className="ml-auto text-muted-foreground">{sel.length} sélectionné{sel.length > 1 ? "s" : ""}</span>
        </div>
        <div className="max-h-[46vh] overflow-y-auto rounded-md border border-border">
          {langs.map((t) => (
            <label key={t.lang} className="flex cursor-default items-center gap-3 border-b border-border/60 px-3 py-2 last:border-b-0 hover:bg-row-hover">
              <Checkbox checked={sel.includes(t.lang)} onCheckedChange={() => toggle(t.lang)} />
              <span className="num w-14 text-[12px]">{t.lang}</span>
              <span className="selectable flex-1 truncate text-[13px]">{t.name ?? ""}</span>
              {t.auto && <Badge variant="secondary" className="text-[10px]">auto</Badge>}
              <span className="num text-[11px] uppercase text-muted-foreground">{t.ext}</span>
            </label>
          ))}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => attempt(ipc.answer(task.id, { Subtitles: { langs: [] } }))}>Sans sous-titres</Button>
          <Button onClick={() => attempt(ipc.answer(task.id, { Subtitles: { langs: sel } }))}>Continuer</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
