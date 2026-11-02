import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Copy, Loader2, Search, X } from "lucide-react";
import { ipc, errMsg } from "@/lib/ipc";
import type { SiteList } from "@/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { copyText, useDebounced } from "./util";

const PAGE = 400;
const nf = new Intl.NumberFormat("fr-FR");

export function SitesView() {
  const [filter, setFilter] = useState("");
  const df = useDebounced(filter, 250);
  const [data, setData] = useState<SiteList | null>(null);
  const [loading, setLoading] = useState(true);
  const [limit, setLimit] = useState(PAGE);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);
  const total = useRef<number | null>(null);
  const reqId = useRef(0);

  useEffect(() => {
    const id = ++reqId.current;
    setLoading(true);
    setError("");
    ipc.sites(df)
      .then((r) => {
        if (id !== reqId.current) return;
        if (df === "") total.current = r.total;
        setData(r);
        setLimit(PAGE);
      })
      .catch((e) => {
        if (id !== reqId.current) return;
        const m = errMsg(e);
        setError(m);
        toast.error(m);
      })
      .finally(() => { if (id === reqId.current) setLoading(false); });
  }, [df, tick]);

  const sites = data?.sites ?? [];
  const shown = sites.slice(0, limit);
  const first = data === null && loading;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 px-3 py-2 hairline">
        <div className="relative w-72">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filtrer les sites" aria-label="Filtrer les sites" className="h-8 pl-8 pr-7 text-[13px]" />
          {filter && (
            <button onClick={() => setFilter("")} aria-label="Effacer le filtre" className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded p-1 text-muted-foreground hover:text-foreground">
              <X className="size-3" />
            </button>
          )}
        </div>
        {loading && data !== null && <Loader2 className="size-3.5 animate-spin text-muted-foreground" />}
        <div className="flex-1" />
        <span className="num text-[12px] text-muted-foreground">
          {data ? `${nf.format(data.sites.length)} sites sur ${nf.format(total.current ?? data.total)}` : ""}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {first && (
          <div className="flex h-full flex-col items-center justify-center gap-1.5 px-8 text-center">
            <Loader2 className="size-5 animate-spin text-muted-foreground" />
            <p className="text-sm font-medium">Chargement de la liste des sites…</p>
            <p className="max-w-sm text-[13px] text-muted-foreground">yt-dlp établit la liste de ses extracteurs, cela prend quelques secondes la première fois.</p>
          </div>
        )}
        {error && data === null && (
          <div className="flex h-full flex-col items-center justify-center gap-1 px-8 text-center">
            <p className="text-sm font-medium">Liste des sites indisponible</p>
            <p className="max-w-sm text-[13px] text-muted-foreground">{error}</p>
            <Button size="sm" variant="secondary" className="mt-2" onClick={() => setTick((t) => t + 1)}>Réessayer</Button>
          </div>
        )}
        {data && sites.length === 0 && !loading && (
          <div className="flex h-full flex-col items-center justify-center gap-1 px-8 text-center">
            <p className="text-sm font-medium">{`Aucun site ne correspond à « ${df} »`}</p>
            <p className="max-w-sm text-[13px] text-muted-foreground">Essayez un nom plus court. Un site absent de la liste peut quand même fonctionner via le navigateur guidé.</p>
          </div>
        )}
        {shown.map((s, i) => (
          <div key={`${s}-${i}`} className="group flex h-8 items-center justify-between px-3 text-[13px] hover:bg-row-hover">
            <span className="selectable truncate">{s}</span>
            <Button size="icon-xs" variant="ghost" className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" aria-label={`Copier ${s}`} title="Copier" onClick={() => void copyText(s, "Nom copié")}>
              <Copy />
            </Button>
          </div>
        ))}
        {sites.length > limit && (
          <div className="flex justify-center p-3">
            <Button size="sm" variant="secondary" onClick={() => setLimit((l) => l + PAGE)}>
              Afficher la suite ({nf.format(sites.length - limit)} restants)
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
