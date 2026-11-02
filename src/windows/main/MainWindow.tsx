import { useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { Titlebar } from "@/components/app/Titlebar";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { AddBar, addFromText } from "./AddBar";
import { Toolbar } from "./Toolbar";
import { QueueList } from "./QueueList";
import { FirstRun, ToolsBanner } from "./Bootstrap";
import { QuestionHost } from "./dialogs/QuestionHost";
import { LinkDetailsDialog } from "./dialogs/LinkDetailsDialog";
import { AddOptionsDialog } from "./dialogs/AddOptionsDialog";
import { ScheduleDialog } from "./dialogs/ScheduleDialog";
import { TgiConfirmDialog } from "./dialogs/TgiConfirmDialog";
import { useQueue } from "@/stores/queue";
import { useUi } from "@/stores/ui";
import { on, onGrabEvent, subscribe } from "@/app/events";
import { ipc, errMsg } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import type { IntegrationInfo } from "@/types";
import { useState } from "react";

export default function MainWindow() {
  const load = useQueue((s) => s.load);
  const apply = useQueue((s) => s.apply);
  const ui = useUi();
  const [ready, setReady] = useState(false);

  useEffect(() => {
    load();
    const off = subscribe(
      onGrabEvent(apply),
      on("grab://lagged", () => load()),
      on<string>("app://error", (m) => toast.error(m)),
      on<{ json: string; info: IntegrationInfo }>("tgi://confirm", (d) => useUi.getState().set({ tgi: d })),
      on<string>("menu://action", (id) => menuAction(id)),
    );
    return off;
  }, [load, apply]);

  useEffect(() => {
    // Collage direct : ⌘V hors d'un champ ajoute les liens du presse-papiers.
    const onPaste = (e: ClipboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && t.closest("input, textarea, [contenteditable]")) return;
      const text = e.clipboardData?.getData("text") ?? "";
      if (text) { e.preventDefault(); addFromText(text); }
    };
    // Glisser-déposer : liens, texte ou fichiers (liste de liens, .tgi).
    const onOver = (e: DragEvent) => { if (!e.dataTransfer?.types.includes("application/x-tribal-task")) e.preventDefault(); };
    const onDrop = async (e: DragEvent) => {
      if (e.dataTransfer?.types.includes("application/x-tribal-task")) return;
      e.preventDefault();
      const dt = e.dataTransfer;
      if (!dt) return;
      const files = [...dt.files];
      if (files.length > 0) {
        for (const f of files) {
          if (f.size > 5 * 1024 * 1024) { toast.error(`${f.name} est trop volumineux.`); continue; }
          const text = await f.text();
          if (f.name.toLowerCase().endsWith(".tgi")) {
            try { useUi.getState().set({ tgi: { json: text, info: JSON.parse(text) as IntegrationInfo } }); } catch { toast.error("Fichier .tgi invalide."); }
          } else await addFromText(text);
        }
        return;
      }
      const text = dt.getData("text/uri-list") || dt.getData("text/plain");
      if (text) await addFromText(text.replace(/^#.*$/gm, ""));
    };
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && t.closest("input, textarea, [contenteditable], [role=dialog]")) return;
      const q = useQueue.getState();
      if (e.key === "Backspace" || e.key === "Delete") { q.selection.forEach((id) => attempt(ipc.remove(id))); }
      else if (e.key === "a" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); q.selectAll(); }
      else if (e.key === "Escape") q.clearSelection();
      else if (e.key === " " && q.selection.length === 1) {
        const t1 = q.tasks[q.selection[0]];
        if (t1?.output_path) { e.preventDefault(); attempt(ipc.quicklook(t1.output_path)); }
      }
    };
    document.addEventListener("paste", onPaste);
    window.addEventListener("dragover", onOver);
    window.addEventListener("drop", onDrop);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("paste", onPaste);
      window.removeEventListener("dragover", onOver);
      window.removeEventListener("drop", onDrop);
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  async function menuAction(id: string) {
    const set = useUi.getState().set;
    if (id === "add_link") useUi.getState().focusAdd();
    else if (id === "paste_download") {
      const text = await ipc.readClipboard();
      if (!(await addFromText(text))) { /* le toast d'erreur est déjà affiché */ }
    } else if (id === "import_file") {
      const r = await open({ multiple: false, title: "Importer des liens ou une intégration", filters: [{ name: "Liens ou intégration", extensions: ["txt", "csv", "html", "md", "tgi"] }] });
      if (typeof r !== "string") return;
      if (r.toLowerCase().endsWith(".tgi")) {
        try { const d = await ipc.integrationReadFile(r); set({ tgi: { json: d.json, info: d.info } }); } catch (e) { toast.error(errMsg(e)); }
      } else {
        const ids = await attempt(ipc.importLinksFile(r));
        if (ids) toast.success(`${ids.length} lien${ids.length > 1 ? "s" : ""} ajouté${ids.length > 1 ? "s" : ""} à la file`);
      }
    } else if (id === "cancel_all" || id === "reset_queue") set({ confirm: id });
  }

  const confirm = ui.confirm;
  return (
    <div className="flex h-full flex-col bg-background">
      <Titlebar><AddBar /></Titlebar>
      <ToolsBanner />
      <Toolbar />
      <QueueList onDetails={(url) => ui.set({ detailsUrl: url })} onSchedule={(id) => ui.set({ scheduleId: id })} />
      <QuestionHost />
      <LinkDetailsDialog url={ui.detailsUrl} onClose={() => ui.set({ detailsUrl: null })} />
      <AddOptionsDialog open={ui.addOptionsOpen} initialText={ui.addOptionsText} onClose={() => ui.set({ addOptionsOpen: false })} />
      <ScheduleDialog id={ui.scheduleId} onClose={() => ui.set({ scheduleId: null })} />
      <TgiConfirmDialog data={ui.tgi} onClose={() => ui.set({ tgi: null })} />
      <AlertDialog open={!!confirm} onOpenChange={(o) => !o && ui.set({ confirm: null })}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{confirm === "reset_queue" ? "Réinitialiser la file ?" : "Tout annuler ?"}</AlertDialogTitle>
            <AlertDialogDescription>
              {confirm === "reset_queue" ? "Tous les téléchargements sont retirés de la file, y compris ceux en cours. Les fichiers déjà terminés ne sont pas supprimés." : "Tous les téléchargements en attente ou en cours sont annulés."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Annuler</AlertDialogCancel>
            <AlertDialogAction onClick={() => attempt(confirm === "reset_queue" ? ipc.resetQueue() : ipc.cancelAll())}>Confirmer</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      {!ready && <FirstRun onDone={() => setReady(true)} />}
    </div>
  );
}
