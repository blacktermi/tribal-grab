import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { on, subscribe } from "@/app/events";
import { toast } from "sonner";
import { Download, Link2, Loader2, Search, TriangleAlert } from "lucide-react";
import { ipc, errMsg } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import { Thumb } from "@/components/app/Thumb";
import { duration, hostOf } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useSettings } from "@/stores/settings";
import type { SearchResult } from "@/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { copyText, plural } from "./util";

type Status = "idle" | "loading" | "done" | "error";

export function SearchView() {
  const engines = useSettings((s) => s.settings?.search_engines);
  const [query, setQuery] = useState("");
  const [asked, setAsked] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const anchor = useRef<number | null>(null);
  const reqId = useRef(0);

  const run = async (override?: string) => {
    const q = (typeof override === "string" ? override : query).trim();
    if (!q) return;
    const id = ++reqId.current;
    setStatus("loading");
    setAsked(q);
    setSelected(new Set());
    anchor.current = null;
    try {
      const list = await ipc.search(q);
      if (id !== reqId.current) return;
      setResults(list);
      setStatus("done");
    } catch (e) {
      if (id !== reqId.current) return;
      const m = errMsg(e);
      setResults([]);
      setError(m);
      setStatus("error");
      toast.error(m);
    }
  };

  // Une recherche lancée depuis la barre d'ajout de la fenêtre principale.
  useEffect(() => subscribe(on<string>("library://query", (q) => { setQuery(q); void run(q); })), []); // eslint-disable-line react-hooks/exhaustive-deps

  const add = (urls: string[]) => {
    void (async () => {
      let n = 0;
      for (const u of urls) { if (await attempt(ipc.addUrl(u)) !== undefined) n++; }
      if (n > 0) toast.success(n > 1 ? `${n} liens ajoutés à la file` : "Ajouté à la file");
    })();
  };

  const pick = (i: number, e: MouseEvent) => {
    const r = results[i];
    if (!r) return;
    if (e.shiftKey && anchor.current != null) {
      const [lo, hi] = anchor.current < i ? [anchor.current, i] : [i, anchor.current];
      const range = results.slice(lo, hi + 1).map((x) => x.url);
      setSelected((s) => new Set(e.metaKey || e.ctrlKey ? [...s, ...range] : range));
      return;
    }
    anchor.current = i;
    setSelected((s) => {
      if (e.metaKey || e.ctrlKey) { const n = new Set(s); if (n.has(r.url)) n.delete(r.url); else n.add(r.url); return n; }
      return new Set([r.url]);
    });
  };

  const chosen = useMemo(() => results.filter((r) => selected.has(r.url)).map((r) => r.url), [results, selected]);
  const engineText = engines && engines.length > 0 ? engines.join(", ") : "aucun";

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 px-3 py-2 hairline">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void run(); }}
            placeholder="Rechercher des vidéos, puis appuyez sur Entrée"
            aria-label="Rechercher des vidéos"
            className="h-8 pl-8 text-[13px]"
          />
        </div>
        <Button size="sm" onClick={() => void run()} disabled={!query.trim() || status === "loading"}>
          {status === "loading" ? <Loader2 className="animate-spin" /> : <Search />} Rechercher
        </Button>
        <Button size="sm" variant="secondary" disabled={chosen.length === 0} onClick={() => add(chosen)}>
          <Download /> Télécharger la sélection{chosen.length > 0 ? ` (${chosen.length})` : ""}
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto" onClick={(e) => { if (e.target === e.currentTarget) setSelected(new Set()); }}>
        {status === "idle" && (
          <State title="Trouvez une vidéo sans quitter l'application" text="Saisissez des mots-clés : les moteurs actifs sont interrogés et chaque résultat s'ajoute à la file d'un clic." />
        )}
        {status === "loading" && (
          <State icon={<Loader2 className="size-5 animate-spin text-muted-foreground" />} title={`Recherche de « ${asked} »…`} text="Cela peut prendre quelques secondes." />
        )}
        {status === "error" && (
          <State icon={<TriangleAlert className="size-5 text-destructive" />} title="La recherche a échoué" text={error} />
        )}
        {status === "done" && results.length === 0 && (
          <State title={`Aucun résultat pour « ${asked} »`} text="Essayez d'autres mots-clés, ou activez un autre moteur dans les préférences." />
        )}
        {status === "done" && results.map((r, i) => (
          <div
            key={`${r.url}-${i}`}
            role="option"
            aria-selected={selected.has(r.url)}
            onClick={(e) => pick(i, e)}
            className={cn("group flex cursor-default select-none items-center gap-3 px-3 py-2 hover:bg-row-hover", selected.has(r.url) && "bg-row-selected hover:bg-row-selected")}
          >
            <Thumb src={r.thumbnail} className="aspect-video w-32" />
            <div className="min-w-0 flex-1">
              <div className="line-clamp-2 text-[13px] leading-[1.3] font-medium" title={r.title}>{r.title}</div>
              <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[12px] text-muted-foreground">
                {r.uploader && <span className="max-w-[240px] truncate">{r.uploader}</span>}
                {r.duration != null && <span className="num">{duration(r.duration)}</span>}
                <Badge variant="secondary" className="h-4 px-1.5 text-[10.5px] font-normal">{r.engine}</Badge>
                <span className="truncate">{hostOf(r.url)}</span>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button size="sm" variant="outline" onClick={(e) => { e.stopPropagation(); add([r.url]); }}><Download /> Télécharger</Button>
              <Button size="icon-sm" variant="ghost" aria-label="Copier le lien" title="Copier le lien" onClick={(e) => { e.stopPropagation(); void copyText(r.url); }}><Link2 /></Button>
            </div>
          </div>
        ))}
      </div>

      <div className="flex h-8 shrink-0 items-center justify-between gap-3 px-3 text-[12px] text-muted-foreground" style={{ boxShadow: "inset 0 1px 0 var(--border)" }}>
        <span className="truncate">
          Moteurs actifs : {engineText}
          {" · "}
          <button className="text-primary underline-offset-2 outline-none hover:underline focus-visible:underline" onClick={() => void attempt(ipc.openWindow("prefs", "moteurs"))}>
            Régler les moteurs
          </button>
        </span>
        <span className="num shrink-0">{status === "done" ? plural(results.length, "résultat", "résultats") : ""}</span>
      </div>
    </div>
  );
}

function State({ icon, title, text }: { icon?: React.ReactNode; title: string; text: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1.5 px-8 text-center">
      {icon}
      <p className="text-sm font-medium">{title}</p>
      <p className="max-w-sm text-[13px] text-muted-foreground">{text}</p>
    </div>
  );
}
