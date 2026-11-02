import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import {
  ArrowDown, ArrowUp, ArrowUpDown, Download, ExternalLink, Eye, FileX, FolderSearch, LayoutGrid, Link2,
  List, MoreHorizontal, RotateCw, Search, Trash2, Upload, X,
} from "lucide-react";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import { Thumb } from "@/components/app/Thumb";
import { bytes, dateTime, hostOf } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { HistoryItem } from "@/types";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import {
  ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuShortcut, ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuRadioGroup,
  DropdownMenuRadioItem, DropdownMenuSeparator, DropdownMenuSub, DropdownMenuSubContent, DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { IconButton } from "./IconButton";
import { copyText, plural, readStore, useDebounced, writeStore } from "./util";

type SortKey = "date" | "title" | "site" | "size";
type ViewMode = "grid" | "table";
type Format = "json" | "csv" | "xml";

const SORT_LABEL: Record<SortKey, string> = { date: "Date", title: "Titre", site: "Site", size: "Taille" };
const VIEW_KEY = "grab.library.view";
const TABLE_COLS = "grid-cols-[72px_minmax(0,1fr)_130px_150px_84px_116px]";

const hasFile = (i: HistoryItem) => !!i.path && i.exists;
const isEditable = (t: EventTarget | null) => t instanceof HTMLElement && !!t.closest("input, textarea, [contenteditable=true]");

export function HistoryView() {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [query, setQuery] = useState("");
  const dq = useDebounced(query, 250);
  const [sort, setSort] = useState<SortKey>("date");
  const [desc, setDesc] = useState(true);
  const [view, setView] = useState<ViewMode>(() => (readStore(VIEW_KEY) === "table" ? "table" : "grid"));
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [confirmClear, setConfirmClear] = useState(false);
  const anchor = useRef<number | null>(null);
  const reqId = useRef(0);
  const box = useRef<HTMLDivElement>(null);

  const reload = useCallback(async () => {
    const id = ++reqId.current;
    try {
      const list = await ipc.history(dq, sort, desc, 2000);
      if (id !== reqId.current) return;
      setItems(list);
      setSelected((s) => {
        const ids = new Set(list.map((i) => i.id));
        const next = new Set([...s].filter((x) => ids.has(x)));
        return next.size === s.size ? s : next;
      });
    } catch (e) {
      if (id === reqId.current) toast.error(typeof e === "string" ? e : "Impossible de lire l'historique");
    } finally {
      if (id === reqId.current) setLoaded(true);
    }
  }, [dq, sort, desc]);

  useEffect(() => { void reload(); }, [reload]);
  useEffect(() => () => { void ipc.closeQuicklook().catch(() => {}); }, []);

  const setViewMode = (v: ViewMode) => { setView(v); writeStore(VIEW_KEY, v); };
  const chooseSort = (k: SortKey) => {
    if (k === sort) setDesc((d) => !d);
    else { setSort(k); setDesc(k === "date" || k === "size"); }
  };

  const targets = useMemo(() => items.filter((i) => selected.has(i.id)), [items, selected]);
  const files = useMemo(() => targets.filter(hasFile), [targets]);

  // Sélection
  const pick = (item: HistoryItem, e: MouseEvent) => {
    if (e.shiftKey && anchor.current != null) {
      const a = items.findIndex((i) => i.id === anchor.current);
      const b = items.findIndex((i) => i.id === item.id);
      if (a >= 0 && b >= 0) {
        const [lo, hi] = a < b ? [a, b] : [b, a];
        const range = items.slice(lo, hi + 1).map((i) => i.id);
        setSelected((s) => new Set(e.metaKey || e.ctrlKey ? [...s, ...range] : range));
        return;
      }
    }
    anchor.current = item.id;
    if (e.metaKey || e.ctrlKey) {
      setSelected((s) => { const n = new Set(s); if (n.has(item.id)) n.delete(item.id); else n.add(item.id); return n; });
    } else {
      setSelected(new Set([item.id]));
    }
  };
  const onContext = (item: HistoryItem) => {
    if (!selected.has(item.id)) { anchor.current = item.id; setSelected(new Set([item.id])); }
  };

  // Actions
  const reveal = () => { if (files[0]?.path) void attempt(ipc.revealInFinder(files[0].path)); };
  const openFiles = () => { files.forEach((f) => { if (f.path) void attempt(ipc.openPath(f.path)); }); };
  const preview = () => { if (files[0]?.path) void attempt(ipc.quicklook(files[0].path)); };
  const redownload = async () => {
    let n = 0;
    for (const t of targets) { if (await attempt(ipc.historyRedownload(t.id)) !== undefined) n++; }
    if (n > 0) toast.success(n > 1 ? `${n} téléchargements ajoutés à la file` : "Ajouté à la file");
  };
  const copyLinks = () => { void copyText(targets.map((t) => t.url).join("\n"), targets.length > 1 ? "Liens copiés" : "Lien copié"); };
  const remove = async () => {
    const ids = targets.map((t) => t.id);
    if (ids.length === 0) return;
    if (await attempt(ipc.historyDelete(ids)) !== undefined) { setSelected(new Set()); await reload(); }
  };
  const removeMissing = async () => {
    const n = await attempt(ipc.historyRemoveMissing());
    if (n === undefined) return;
    toast.success(n === 0 ? "Aucun fichier supprimé à retirer" : `${plural(n, "élément retiré", "éléments retirés")}`);
    await reload();
  };
  const clearAll = async () => {
    if (await attempt(ipc.historyClear(), "Historique effacé") !== undefined) { setSelected(new Set()); await reload(); }
  };
  const exportAs = async (format: Format) => {
    try {
      const path = await saveDialog({
        title: "Exporter l'historique",
        defaultPath: `historique.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!path) return;
      await attempt(ipc.historyExportFile(format, path), "Historique exporté");
    } catch { toast.error("Impossible d'ouvrir le dialogue d'enregistrement"); }
  };
  const importFile = async () => {
    try {
      const path = await openDialog({
        title: "Importer un historique",
        multiple: false,
        directory: false,
        filters: [{ name: "Historique", extensions: ["json", "csv", "xml"] }],
      });
      if (typeof path !== "string") return;
      const ext = path.split(".").pop()?.toLowerCase();
      if (ext !== "json" && ext !== "csv" && ext !== "xml") { toast.error("Format non reconnu : choisissez un fichier .json, .csv ou .xml"); return; }
      const n = await attempt(ipc.historyImportFile(ext, path));
      if (n === undefined) return;
      toast.success(`${plural(n, "élément importé", "éléments importés")}`);
      await reload();
    } catch { toast.error("Impossible d'ouvrir le dialogue d'ouverture"); }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (isEditable(e.target)) return;
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a") {
      e.preventDefault();
      setSelected(new Set(items.map((i) => i.id)));
    } else if (e.key === " ") {
      e.preventDefault();
      preview();
    } else if (e.key === "Escape") {
      setSelected(new Set());
    } else if (e.key === "Enter" && files.length > 0) {
      openFiles();
    }
  };

  const none = targets.length === 0;
  const menu = (
    <ItemMenu
      canFile={files.length > 0}
      count={targets.length}
      onReveal={reveal} onOpen={openFiles} onPreview={preview}
      onRedownload={() => void redownload()} onCopy={copyLinks} onDelete={() => void remove()}
    />
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Barre d'outils */}
      <div className="flex shrink-0 items-center gap-1.5 px-3 py-2 hairline">
        <div className="relative w-64">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Rechercher dans l'historique"
            aria-label="Rechercher dans l'historique"
            className="h-8 pl-8 pr-7 text-[13px]"
          />
          {query && (
            <button onClick={() => setQuery("")} aria-label="Effacer la recherche" className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded p-1 text-muted-foreground hover:text-foreground">
              <X className="size-3" />
            </button>
          )}
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="flex h-8 items-center gap-1.5 rounded-md px-2.5 text-[13px] text-muted-foreground outline-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring/50">
              <ArrowUpDown className="size-3.5" />
              {SORT_LABEL[sort]}
              {desc ? <ArrowDown className="size-3" /> : <ArrowUp className="size-3" />}
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-44">
            <DropdownMenuLabel>Trier par</DropdownMenuLabel>
            <DropdownMenuRadioGroup value={sort} onValueChange={(v) => chooseSort(v as SortKey)}>
              {(Object.keys(SORT_LABEL) as SortKey[]).map((k) => (
                <DropdownMenuRadioItem key={k} value={k}>{SORT_LABEL[k]}</DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
            <DropdownMenuSeparator />
            <DropdownMenuRadioGroup value={desc ? "desc" : "asc"} onValueChange={(v) => setDesc(v === "desc")}>
              <DropdownMenuRadioItem value="asc">Croissant</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="desc">Décroissant</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>

        <div className="flex-1" />

        <IconButton label="Afficher dans le Finder" disabled={files.length === 0} onClick={reveal}><FolderSearch /></IconButton>
        <IconButton label="Ouvrir" disabled={files.length === 0} onClick={openFiles}><ExternalLink /></IconButton>
        <IconButton label="Aperçu (espace)" disabled={files.length === 0} onClick={preview}><Eye /></IconButton>
        <IconButton label="Retélécharger" disabled={none} onClick={() => void redownload()}><RotateCw /></IconButton>
        <IconButton label="Copier le lien" disabled={none} onClick={copyLinks}><Link2 /></IconButton>
        <IconButton label="Supprimer de l'historique" disabled={none} onClick={() => void remove()}><Trash2 /></IconButton>

        <Separator orientation="vertical" className="mx-1 h-5" />

        <div className="flex items-center rounded-md bg-muted p-0.5">
          <SegButton active={view === "grid"} label="Grille" onClick={() => setViewMode("grid")}><LayoutGrid className="size-3.5" /></SegButton>
          <SegButton active={view === "table"} label="Tableau" onClick={() => setViewMode("table")}><List className="size-3.5" /></SegButton>
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton label="Plus"><MoreHorizontal /></IconButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-60">
            <DropdownMenuItem onSelect={() => void removeMissing()}><FileX /> Retirer les fichiers supprimés</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuSub>
              <DropdownMenuSubTrigger><Download /> Exporter…</DropdownMenuSubTrigger>
              <DropdownMenuSubContent>
                <DropdownMenuItem onSelect={() => void exportAs("json")}>JSON</DropdownMenuItem>
                <DropdownMenuItem onSelect={() => void exportAs("csv")}>CSV</DropdownMenuItem>
                <DropdownMenuItem onSelect={() => void exportAs("xml")}>XML</DropdownMenuItem>
              </DropdownMenuSubContent>
            </DropdownMenuSub>
            <DropdownMenuItem onSelect={() => void importFile()}><Upload /> Importer…</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem variant="destructive" onSelect={() => setConfirmClear(true)}><Trash2 /> Effacer tout l'historique…</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {/* Contenu */}
      <div
        ref={box}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onClick={(e) => { if (e.target === e.currentTarget) setSelected(new Set()); }}
        className="min-h-0 flex-1 overflow-y-auto outline-none"
      >
        {!loaded ? null : items.length === 0 ? (
          <Empty query={dq} />
        ) : view === "grid" ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(188px,1fr))] gap-3 p-3" onClick={(e) => { if (e.target === e.currentTarget) setSelected(new Set()); }}>
            {items.map((it) => (
              <ContextMenu key={it.id}>
                <ContextMenuTrigger asChild>
                  <div
                    role="option"
                    aria-selected={selected.has(it.id)}
                    onClick={(e) => pick(it, e)}
                    onContextMenu={() => onContext(it)}
                    onDoubleClick={() => { if (hasFile(it) && it.path) void attempt(ipc.openPath(it.path)); }}
                    className={cn(
                      "group cursor-default select-none rounded-lg p-1.5 transition-colors hover:bg-row-hover",
                      selected.has(it.id) && "bg-row-selected hover:bg-row-selected ring-1 ring-primary/60",
                    )}
                  >
                    <div className="relative">
                      <Thumb src={it.thumbnail} className={cn("aspect-video w-full", !it.exists && "opacity-40 grayscale")} />
                    </div>
                    <div className={cn("mt-1.5 line-clamp-2 min-h-[2.5em] text-[13px] leading-[1.25] font-medium", !it.exists && "text-muted-foreground")} title={it.title}>
                      {it.title || it.url}
                    </div>
                    <div className="mt-0.5 flex items-center justify-between gap-2 text-[11.5px] text-muted-foreground">
                      <span className="truncate">{it.site || hostOf(it.url)}</span>
                      <span className="num shrink-0">{bytes(it.size)}</span>
                    </div>
                    <div className="mt-0.5 flex items-center justify-between gap-2 text-[11.5px] text-muted-foreground">
                      <span className="num truncate">{dateTime(it.finished_at)}</span>
                    </div>
                    {!it.exists && <div className="mt-1 text-[11.5px] text-destructive">Fichier supprimé</div>}
                  </div>
                </ContextMenuTrigger>
                {menu}
              </ContextMenu>
            ))}
          </div>
        ) : (
          <div role="listbox" aria-multiselectable="true" className="text-[13px]">
            <div className={cn("sticky top-0 z-10 grid items-center gap-3 bg-background px-3 py-1.5 text-[11.5px] font-medium text-muted-foreground hairline", TABLE_COLS)}>
              <span />
              <HeadCell k="title" label="Titre" sort={sort} desc={desc} onSort={chooseSort} />
              <HeadCell k="site" label="Site" sort={sort} desc={desc} onSort={chooseSort} />
              <HeadCell k="date" label="Date" sort={sort} desc={desc} onSort={chooseSort} />
              <HeadCell k="size" label="Taille" sort={sort} desc={desc} onSort={chooseSort} right />
              <span>Fichier</span>
            </div>
            {items.map((it) => (
              <ContextMenu key={it.id}>
                <ContextMenuTrigger asChild>
                  <div
                    role="option"
                    aria-selected={selected.has(it.id)}
                    onClick={(e) => pick(it, e)}
                    onContextMenu={() => onContext(it)}
                    onDoubleClick={() => { if (hasFile(it) && it.path) void attempt(ipc.openPath(it.path)); }}
                    className={cn(
                      "grid cursor-default select-none items-center gap-3 px-3 py-1.5 hover:bg-row-hover",
                      TABLE_COLS,
                      selected.has(it.id) && "bg-row-selected hover:bg-row-selected",
                      !it.exists && "text-muted-foreground",
                    )}
                  >
                    <Thumb src={it.thumbnail} className={cn("h-[36px] w-16", !it.exists && "opacity-40 grayscale")} />
                    <span className="truncate" title={it.title}>{it.title || it.url}</span>
                    <span className="truncate text-muted-foreground">{it.site || hostOf(it.url)}</span>
                    <span className="num truncate text-muted-foreground">{dateTime(it.finished_at)}</span>
                    <span className="num text-right text-muted-foreground">{bytes(it.size)}</span>
                    <span className={cn("truncate text-[12px]", it.path && !it.exists ? "text-destructive" : "text-muted-foreground")}>
                      {!it.path ? "Sans fichier" : it.exists ? "Présent" : "Fichier supprimé"}
                    </span>
                  </div>
                </ContextMenuTrigger>
                {menu}
              </ContextMenu>
            ))}
          </div>
        )}
      </div>

      {/* Pied */}
      <div className="flex h-8 shrink-0 items-center justify-between px-3 text-[12px] text-muted-foreground" style={{ boxShadow: "inset 0 1px 0 var(--border)" }}>
        <span className="num">{plural(items.length, "élément", "éléments")}{dq ? " (filtrés)" : ""}</span>
        <span className="num">{targets.length > 0 ? plural(targets.length, "sélectionné", "sélectionnés") : "⌘-clic et Maj-clic pour sélectionner plusieurs éléments"}</span>
      </div>

      <AlertDialog open={confirmClear} onOpenChange={setConfirmClear}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Effacer tout l'historique ?</AlertDialogTitle>
            <AlertDialogDescription>
              Les {plural(items.length, "entrée", "entrées")} de l'historique seront supprimées. Les fichiers déjà téléchargés restent sur votre disque.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Annuler</AlertDialogCancel>
            <AlertDialogAction onClick={() => void clearAll()}>Tout effacer</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function SegButton({ active, label, onClick, children }: { active: boolean; label: string; onClick: () => void; children: ReactNode }) {
  return (
    <IconButton
      label={label}
      onClick={onClick}
      aria-pressed={active}
      className={cn("size-7 rounded-[5px]", active ? "bg-background text-foreground shadow-xs hover:bg-background" : "text-muted-foreground")}
    >
      {children}
    </IconButton>
  );
}

function HeadCell({ k, label, sort, desc, onSort, right }: { k: SortKey; label: string; sort: SortKey; desc: boolean; onSort: (k: SortKey) => void; right?: boolean }) {
  const on = sort === k;
  return (
    <button onClick={() => onSort(k)} className={cn("flex items-center gap-1 outline-none hover:text-foreground", right && "justify-end", on && "text-foreground")}>
      {label}
      {on && (desc ? <ArrowDown className="size-3" /> : <ArrowUp className="size-3" />)}
    </button>
  );
}

function ItemMenu(p: {
  canFile: boolean; count: number;
  onReveal: () => void; onOpen: () => void; onPreview: () => void;
  onRedownload: () => void; onCopy: () => void; onDelete: () => void;
}) {
  return (
    <ContextMenuContent className="w-60">
      <ContextMenuItem disabled={!p.canFile} onSelect={p.onReveal}><FolderSearch /> Afficher dans le Finder</ContextMenuItem>
      <ContextMenuItem disabled={!p.canFile} onSelect={p.onOpen}><ExternalLink /> Ouvrir</ContextMenuItem>
      <ContextMenuItem disabled={!p.canFile} onSelect={p.onPreview}><Eye /> Aperçu <ContextMenuShortcut>espace</ContextMenuShortcut></ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem onSelect={p.onRedownload}><RotateCw /> Retélécharger</ContextMenuItem>
      <ContextMenuItem onSelect={p.onCopy}><Link2 /> Copier le lien</ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem variant="destructive" onSelect={p.onDelete}><Trash2 /> Supprimer de l'historique</ContextMenuItem>
    </ContextMenuContent>
  );
}

function Empty({ query }: { query: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1 px-8 text-center">
      <p className="text-sm font-medium">{query ? `Aucun résultat pour « ${query} »` : "L'historique est vide"}</p>
      <p className="max-w-sm text-[13px] text-muted-foreground">
        {query
          ? "Essayez un autre mot du titre, du site ou du lien."
          : "Les téléchargements terminés apparaîtront ici. Collez un lien dans la fenêtre principale pour commencer, ou importez un historique depuis le menu « Plus »."}
      </p>
    </div>
  );
}
