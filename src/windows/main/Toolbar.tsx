import { ArrowDownUp, Ellipsis, Pause, Play, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuRadioGroup,
  DropdownMenuRadioItem, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import { counts, useQueue, type Filter } from "@/stores/queue";
import { useSettings } from "@/stores/settings";
import { speed } from "@/lib/format";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import { useUi } from "@/stores/ui";
import type { SortKey } from "@/types";

const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "Tous" },
  { id: "active", label: "En cours" },
  { id: "waiting", label: "En attente" },
  { id: "finished", label: "Terminés" },
  { id: "failed", label: "Échecs" },
];
const SORTS: { id: SortKey; label: string }[] = [
  { id: "Order", label: "Ordre de la file" },
  { id: "Name", label: "Nom" },
  { id: "Site", label: "Site" },
  { id: "Playlist", label: "Playlist" },
  { id: "Progress", label: "Progression" },
];

export function Toolbar() {
  const tasks = useQueue((s) => s.tasks);
  const filter = useQueue((s) => s.filter);
  const setFilter = useQueue((s) => s.setFilter);
  const sortKey = useQueue((s) => s.sortKey);
  const sortDesc = useQueue((s) => s.sortDesc);
  const setSort = useQueue((s) => s.setSort);
  const autoStart = useSettings((s) => s.settings?.auto_start ?? true);
  const setUi = useUi((s) => s.set);
  const c = counts(tasks);
  const n: Record<Filter, number> = { all: c.all, active: c.active, waiting: c.waiting, finished: c.finished, failed: c.failed };

  return (
    <div className="flex h-10 shrink-0 items-center gap-1 border-b border-border px-3">
      <div className="flex items-center gap-0.5">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            onClick={() => setFilter(f.id)}
            className={cn("flex h-7 items-center gap-1.5 rounded-md px-2 text-[12px] transition-colors", filter === f.id ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent/60 hover:text-foreground")}
          >
            {f.label}
            <span className={cn("num min-w-[1.1em] text-center text-[11px]", f.id === "failed" && n.failed > 0 ? "text-destructive" : f.id === "active" && n.active > 0 ? "text-primary" : "")}>{n[f.id]}</span>
          </button>
        ))}
      </div>
      <div className="num ml-3 hidden w-[96px] text-[12px] text-muted-foreground md:block" title="Débit total">{c.active > 0 ? speed(c.speed) : ""}</div>
      <div className="flex-1" />
      {!autoStart && <Button size="sm" variant="secondary" className="h-7" onClick={() => attempt(ipc.startQueue())}><Play className="size-3.5" />Démarrer la file</Button>}
      <Button size="icon-sm" variant="ghost" className="size-7" title="Tout mettre en pause" onClick={() => attempt(ipc.pauseAll())}><Pause className="size-3.5" /></Button>
      <Button size="icon-sm" variant="ghost" className="size-7" title="Tout reprendre" onClick={() => attempt(ipc.resumeAll())}><Play className="size-3.5" /></Button>
      <Button size="icon-sm" variant="ghost" className="size-7" title="Retirer les terminés" onClick={() => attempt(ipc.clearFinished())}><Trash2 className="size-3.5" /></Button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button size="icon-sm" variant="ghost" className="size-7" title="Trier"><ArrowDownUp className="size-3.5" /></Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-52">
          <DropdownMenuLabel>Trier par</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={sortKey} onValueChange={(v) => setSort(v as SortKey, sortDesc)}>
            {SORTS.map((s) => <DropdownMenuRadioItem key={s.id} value={s.id}>{s.label}</DropdownMenuRadioItem>)}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuRadioGroup value={sortDesc ? "desc" : "asc"} onValueChange={(v) => setSort(sortKey, v === "desc")}>
            <DropdownMenuRadioItem value="asc">Croissant</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="desc">Décroissant</DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button size="icon-sm" variant="ghost" className="size-7" title="Plus d'actions"><Ellipsis className="size-3.5" /></Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-56">
          <DropdownMenuItem onSelect={() => attempt(ipc.startQueue())}>Démarrer la file</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => attempt(ipc.clearInactive())}>Retirer les inactifs</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={() => setUi({ confirm: "cancel_all" })}>Tout annuler…</DropdownMenuItem>
          <DropdownMenuItem variant="destructive" onSelect={() => setUi({ confirm: "reset_queue" })}>Réinitialiser la file…</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
