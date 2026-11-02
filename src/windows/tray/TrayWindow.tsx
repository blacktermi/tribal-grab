import { useEffect, useMemo, useState } from "react";
import { ClipboardPaste, Compass, Settings, Power, ExternalLink } from "lucide-react";
import { Button } from "@/components/ui/button";
import { counts, useQueue } from "@/stores/queue";
import { on, onGrabEvent, subscribe } from "@/app/events";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import { speed, stateText, taskTitle, isActive } from "@/lib/format";
import { toast } from "sonner";

/** Popover de la barre de menus : coller et télécharger, compteurs, extraction guidée. */
export default function TrayWindow() {
  const load = useQueue((s) => s.load);
  const apply = useQueue((s) => s.apply);
  const tasks = useQueue((s) => s.tasks);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    load();
    return subscribe(onGrabEvent(apply), on("popover://shown", () => load()), on("grab://lagged", () => load()));
  }, [load, apply]);

  const c = counts(tasks);
  const rows = useMemo(
    () => Object.values(tasks).filter((t) => !["Finished", "Cancelled"].includes(t.state) || false).sort((a, b) => Number(isActive(b.state)) - Number(isActive(a.state)) || a.position - b.position).slice(0, 6),
    [tasks],
  );

  async function pasteAndDownload() {
    setBusy(true);
    try {
      const text = await ipc.readClipboard();
      const ids = await ipc.addText(text);
      toast.success(ids.length === 1 ? "Lien ajouté" : `${ids.length} liens ajoutés`);
    } catch {
      toast.error("Le presse-papiers ne contient aucun lien.");
    } finally { setBusy(false); }
  }
  const open = (w: "main" | "prefs") => attempt(ipc.openWindow(w));

  return (
    <div className="flex h-full flex-col bg-popover text-popover-foreground">
      <div className="flex items-baseline justify-between px-4 pb-2 pt-3.5">
        <span className="text-[13px] font-semibold">Tribal Grab</span>
        <span className="num text-[12px] text-muted-foreground">{c.active > 0 ? `${c.active} en cours · ${speed(c.speed)}` : "Aucun téléchargement actif"}</span>
      </div>
      <div className="px-3 pb-3">
        <Button className="w-full justify-center" onClick={pasteAndDownload} disabled={busy}><ClipboardPaste className="size-4" />Coller et télécharger</Button>
      </div>
      <div className="num grid grid-cols-4 gap-px border-y border-border bg-border text-center text-[11px]">
        {([["En cours", c.active], ["En attente", c.waiting], ["Terminés", c.finished], ["Échecs", c.failed]] as const).map(([l, n]) => (
          <div key={l} className="bg-popover py-2"><div className={`text-[15px] font-semibold ${l === "Échecs" && n > 0 ? "text-destructive" : l === "En cours" && n > 0 ? "text-primary" : ""}`}>{n}</div><div className="font-sans text-muted-foreground">{l}</div></div>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {rows.length === 0 ? (
          <div className="px-4 py-6 text-center text-[12.5px] text-muted-foreground">La file est vide. Copiez un lien puis cliquez sur « Coller et télécharger ».</div>
        ) : rows.map((t) => (
          <div key={t.id} className="border-b border-border/60 px-4 py-2">
            <div className="truncate text-[12.5px] font-medium">{taskTitle(t)}</div>
            <div className="mt-1 flex items-center gap-2">
              <div className="h-[3px] flex-1 overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-primary transition-[width] duration-500" style={{ width: `${isActive(t.state) ? t.progress.percent : 0}%` }} /></div>
              <span className="num w-[78px] truncate text-right text-[11px] text-muted-foreground">{isActive(t.state) && t.progress.speed ? speed(t.progress.speed) : stateText(t)}</span>
            </div>
          </div>
        ))}
      </div>
      <div className="flex items-center gap-1 border-t border-border p-2">
        <Button variant="ghost" size="sm" className="h-7 flex-1 justify-start gap-1.5 px-2 text-[12px]" onClick={() => open("main")}><ExternalLink className="size-3.5" />Ouvrir</Button>
        <Button variant="ghost" size="sm" className="h-7 flex-1 justify-start gap-1.5 px-2 text-[12px]" onClick={() => attempt(ipc.openWindow("browser"))}><Compass className="size-3.5" />Extraction guidée</Button>
        <Button variant="ghost" size="icon-sm" className="size-7" title="Préférences" onClick={() => open("prefs")}><Settings className="size-3.5" /></Button>
        <Button variant="ghost" size="icon-sm" className="size-7" title="Quitter" onClick={() => ipc.quitApp()}><Power className="size-3.5" /></Button>
      </div>
    </div>
  );
}
