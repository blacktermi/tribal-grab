import { useMemo, useState } from "react";
import { Info } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Thumb } from "@/components/app/Thumb";
import { cn } from "@/lib/utils";
import { bytes, duration, hostOf } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import type { LinkCandidate, Task } from "@/types";
import { useUi } from "@/stores/ui";

/** Filtre de liens : playlist ou page à plusieurs médias. Toute la sélection se fait côté interface. */
export function LinkFilterDialog({ task, title, links }: { task: Task; title: string; links: LinkCandidate[] }) {
  const [checked, setChecked] = useState<Set<number>>(() => new Set(links.map((_, i) => i)));
  const [reversed, setReversed] = useState(false);
  const [asPlaylist, setAsPlaylist] = useState(links.length > 1 && !!task.playlist);
  const [n, setN] = useState("10");
  const [from, setFrom] = useState("1");
  const [to, setTo] = useState(String(Math.min(links.length, 10)));
  const [query, setQuery] = useState("");
  const setUi = useUi((s) => s.set);

  const order = useMemo(() => {
    const idx = links.map((_, i) => i);
    return reversed ? idx.reverse() : idx;
  }, [links, reversed]);
  const q = query.trim().toLowerCase();
  const visible = useMemo(() => (q ? order.filter((i) => `${links[i].title ?? ""} ${links[i].url}`.toLowerCase().includes(q)) : order), [order, q, links]);

  const set = (ids: number[]) => setChecked(new Set(ids));
  const num = (s: string, d: number) => Math.max(0, Math.floor(Number(s) || d));
  const chosen = order.filter((i) => checked.has(i));
  const biggest = () => {
    let best = -1, size = -1;
    links.forEach((l, i) => { if ((l.size ?? -1) > size) { size = l.size ?? -1; best = i; } });
    if (best >= 0) set([best]);
  };

  const btn = "h-7 px-2 text-[12px]";
  return (
    <Dialog open>
      <DialogContent className="flex max-h-[86vh] max-w-3xl flex-col gap-3" showCloseButton={false} onEscapeKeyDown={(e) => e.preventDefault()} onInteractOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <DialogTitle>Choisir les liens à télécharger</DialogTitle>
          <DialogDescription className="selectable truncate">{title || hostOf(task.url)} · {links.length} lien{links.length > 1 ? "s" : ""}</DialogDescription>
        </DialogHeader>

        <div className="flex flex-wrap items-center gap-1.5">
          <Button size="sm" variant="secondary" className={btn} onClick={() => set(links.map((_, i) => i))}>Tout</Button>
          <Button size="sm" variant="secondary" className={btn} onClick={() => set([])}>Rien</Button>
          <Button size="sm" variant="secondary" className={btn} onClick={() => set(links.map((_, i) => i).filter((i) => !checked.has(i)))}>Inverser</Button>
          <span className="mx-1 h-4 w-px bg-border" />
          <Input value={n} onChange={(e) => setN(e.target.value)} className="num h-7 w-12 px-2 text-center text-[12px]" inputMode="numeric" />
          <Button size="sm" variant="secondary" className={btn} onClick={() => set(order.slice(0, num(n, 1)))}>premiers</Button>
          <Button size="sm" variant="secondary" className={btn} onClick={() => set(order.slice(Math.max(0, order.length - num(n, 1))))}>derniers</Button>
          <span className="mx-1 h-4 w-px bg-border" />
          <Input value={from} onChange={(e) => setFrom(e.target.value)} className="num h-7 w-12 px-2 text-center text-[12px]" inputMode="numeric" />
          <span className="text-[12px] text-muted-foreground">à</span>
          <Input value={to} onChange={(e) => setTo(e.target.value)} className="num h-7 w-12 px-2 text-center text-[12px]" inputMode="numeric" />
          <Button size="sm" variant="secondary" className={btn} onClick={() => set(order.slice(Math.max(0, num(from, 1) - 1), num(to, links.length)))}>Plage</Button>
          <span className="mx-1 h-4 w-px bg-border" />
          <Button size="sm" variant="secondary" className={btn} onClick={biggest} disabled={!links.some((l) => l.size)}>Plus gros fichier</Button>
          <Button size="sm" variant="secondary" className={btn} onClick={() => set(links.map((l, i) => (l.recognized ? i : -1)).filter((i) => i >= 0))}>Reconnus</Button>
          <Button size="sm" variant={reversed ? "default" : "secondary"} className={btn} onClick={() => setReversed((r) => !r)}>Inverser l'ordre</Button>
        </div>
        <div className="flex items-center gap-2">
          <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Rechercher dans la liste" className="h-7 text-[12px]" />
          <Button size="sm" variant="secondary" className={btn} disabled={!q} onClick={() => set([...new Set([...checked, ...visible])])}>Cocher les résultats</Button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto rounded-md border border-border">
          {visible.map((i) => {
            const l = links[i];
            return (
              <label key={i} className={cn("flex cursor-default items-center gap-3 border-b border-border/60 px-3 py-1.5 last:border-b-0 hover:bg-row-hover", checked.has(i) && "bg-row-selected/50")}>
                <Checkbox checked={checked.has(i)} onCheckedChange={(v) => setChecked((s) => { const x = new Set(s); if (v) x.add(i); else x.delete(i); return x; })} />
                <span className="num w-8 text-right text-[11px] text-muted-foreground">{i + 1}</span>
                <Thumb src={l.thumbnail} className="h-7 w-12" />
                <span className="selectable min-w-0 flex-1 truncate text-[12.5px]" title={l.url}>{l.title || l.url}</span>
                {!l.recognized && <span className="text-[10.5px] text-muted-foreground">lien brut</span>}
                <span className="num w-14 text-right text-[11.5px] text-muted-foreground">{duration(l.duration)}</span>
                <span className="num w-[68px] text-right text-[11.5px] text-muted-foreground">{l.size ? bytes(l.size) : ""}</span>
                <button type="button" className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground" title="Détails du lien" onClick={(e) => { e.preventDefault(); setUi({ detailsUrl: l.url }); }}><Info className="size-3.5" /></button>
              </label>
            );
          })}
          {visible.length === 0 && <div className="p-6 text-center text-[12.5px] text-muted-foreground">Aucun lien ne correspond à la recherche.</div>}
        </div>

        <DialogFooter className="items-center sm:justify-between">
          <div className="flex items-center gap-3 text-[12px]">
            <div className="inline-flex rounded-md border border-border p-0.5">
              {([[true, "En playlist"], [false, "Individuellement"]] as const).map(([v, label]) => (
                <button key={label} className={cn("rounded px-2 py-1", asPlaylist === v ? "bg-accent text-foreground" : "text-muted-foreground")} onClick={() => setAsPlaylist(v)}>{label}</button>
              ))}
            </div>
            <span className="num text-muted-foreground">{chosen.length} / {links.length}</span>
          </div>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={() => attempt(ipc.remove(task.id))}>Annuler</Button>
            <Button disabled={chosen.length === 0} onClick={() => attempt(ipc.answer(task.id, { Links: { urls: chosen.map((i) => links[i].url), as_playlist: asPlaylist } }))}>
              Ajouter {chosen.length > 0 ? chosen.length : ""}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
