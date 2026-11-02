import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Thumb } from "@/components/app/Thumb";
import { ipc, errMsg } from "@/lib/ipc";
import { bytes, duration, hostOf } from "@/lib/format";
import type { MediaInfo, Resolved } from "@/types";

function Single({ m }: { m: MediaInfo }) {
  const formats = [...m.formats].sort((a, b) => (b.height ?? 0) - (a.height ?? 0) || (b.tbr ?? 0) - (a.tbr ?? 0));
  return (
    <div className="space-y-4">
      <div className="flex gap-3">
        <Thumb src={m.thumbnail} className="h-[72px] w-[128px]" />
        <div className="min-w-0 flex-1 text-[12.5px]">
          <div className="selectable text-[14px] font-semibold leading-snug">{m.title}</div>
          <div className="mt-1 text-muted-foreground">{m.site || m.host}{m.uploader ? ` · ${m.uploader}` : ""}</div>
          <div className="num mt-1 text-muted-foreground">
            {m.duration ? duration(m.duration) : ""}{m.upload_date ? ` · ${m.upload_date}` : ""}{m.is_live ? " · direct" : ""}
          </div>
        </div>
      </div>
      {m.description && <p className="selectable line-clamp-3 text-[12px] text-muted-foreground">{m.description}</p>}
      <div>
        <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">Formats ({formats.length})</div>
        <div className="max-h-52 overflow-y-auto rounded-md border border-border">
          <table className="w-full text-[12px]">
            <thead className="sticky top-0 bg-card text-left text-muted-foreground"><tr><th className="px-2 py-1 font-medium">Qualité</th><th className="px-2 py-1 font-medium">Type</th><th className="px-2 py-1 font-medium">Codecs</th><th className="px-2 py-1 text-right font-medium">Taille</th><th className="px-2 py-1 font-medium">Protocole</th></tr></thead>
            <tbody className="num">
              {formats.map((f) => (
                <tr key={f.id} className="border-t border-border/60">
                  <td className="px-2 py-1">{f.height ? `${f.height}p` : f.acodec && f.acodec !== "none" ? "Audio" : "?"}{f.label ? ` ${f.label}` : ""}{f.language ? ` (${f.language})` : ""}</td>
                  <td className="px-2 py-1">{f.ext}</td>
                  <td className="px-2 py-1 text-muted-foreground">{[f.vcodec, f.acodec].filter((c) => c && c !== "none").join(" + ") || "–"}</td>
                  <td className="px-2 py-1 text-right">{bytes(f.filesize)}</td>
                  <td className="px-2 py-1">{f.protocol}{f.has_drm && <Badge variant="destructive" className="ml-1.5 h-4 px-1 text-[10px]">DRM</Badge>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      {m.subtitles.length > 0 && (
        <div className="text-[12px]">
          <span className="mr-2 font-semibold uppercase tracking-wide text-[11px] text-muted-foreground">Sous-titres</span>
          <span className="selectable">{[...new Set(m.subtitles.map((s) => s.lang + (s.auto ? " (auto)" : "")))].join(", ")}</span>
        </div>
      )}
    </div>
  );
}

/** Détails d'un lien : analyse sans créer de tâche. Tous les textes distants sont rendus comme du texte. */
export function LinkDetailsDialog({ url, onClose }: { url: string | null; onClose: () => void }) {
  const [res, setRes] = useState<Resolved | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    setRes(null);
    setErr(null);
    if (!url) return;
    let alive = true;
    ipc.probe(url).then((r) => alive && setRes(r)).catch((e) => alive && setErr(errMsg(e)));
    return () => { alive = false; };
  }, [url]);
  return (
    <Dialog open={!!url} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Détails du lien</DialogTitle>
          <DialogDescription className="selectable truncate">{url ? `${hostOf(url)} · ${url}` : ""}</DialogDescription>
        </DialogHeader>
        {!res && !err && <div className="flex items-center gap-2 py-8 text-[13px] text-muted-foreground"><Loader2 className="size-4 animate-spin" />Analyse en cours…</div>}
        {err && <div className="selectable rounded-md border border-destructive/40 bg-destructive/10 p-3 text-[13px]">{err}</div>}
        {res && "Single" in res && <Single m={res.Single} />}
        {res && "Playlist" in res && <p className="text-[13px]">Playlist « <span className="selectable">{res.Playlist.title}</span> » : {res.Playlist.entries.length} éléments.</p>}
        {res && "Links" in res && <p className="text-[13px]">Cette page contient {res.Links.length} média{res.Links.length > 1 ? "s" : ""} possible{res.Links.length > 1 ? "s" : ""}.</p>}
        {res && "Unsupported" in res && <p className="selectable text-[13px] text-muted-foreground">Lien non pris en charge : {res.Unsupported}</p>}
      </DialogContent>
    </Dialog>
  );
}
