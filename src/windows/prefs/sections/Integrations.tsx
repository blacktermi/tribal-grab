import { useCallback, useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, FilePlus2, Pencil, Trash2, Upload } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { attempt } from "@/components/app/errors";
import { ipc } from "@/lib/ipc";
import type { IntegrationInfo } from "@/types";
import { btnCls, Page } from "./controls";
import IntegrationEditor from "./IntegrationEditor";

const TEMPLATE_CODE = `// La variable globale \`url\` contient l'adresse à analyser.
grab.log("Analyse de " + url);

const page = grab.fetch(url);
const m = /<title>([^<]*)<\\/title>/i.exec(page.body || "");

grab.result({
  title: m ? m[1] : "Sans titre",
  id: "exemple",
  thumbnail: null,
  formats: [
    { url: "https://exemple.com/video-720.mp4", ext: "mp4", height: 720, protocol: "http" },
    { url: "https://exemple.com/flux.m3u8", ext: "mp4", height: 1080, protocol: "hls" },
  ],
  subtitles: [
    { lang: "fr", url: "https://exemple.com/fr.vtt", ext: "vtt" },
  ],
});
`;

const NEW_INTEGRATION: IntegrationInfo = {
  name: "",
  url_regex: "^https?://(www\\.)?exemple\\.com/",
  enabled: true,
  timeout_secs: 30,
  requires_account: false,
  example_url: "https://exemple.com/video/1",
  comment: null,
  code: TEMPLATE_CODE,
};

interface Pending { json: string; info: IntegrationInfo; path: string }

export default function Integrations() {
  const [list, setList] = useState<IntegrationInfo[]>([]);
  const [editing, setEditing] = useState<{ info: IntegrationInfo; isNew: boolean } | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [toDelete, setToDelete] = useState<IntegrationInfo | null>(null);

  const refresh = useCallback(async () => {
    const l = await attempt(ipc.integrations());
    if (l) setList(l);
  }, []);
  useEffect(() => { void refresh(); }, [refresh]);

  const importFile = async () => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "Intégration Tribal Grab", extensions: ["tgi"] }] });
    if (typeof path !== "string") return;
    const r = await attempt(ipc.integrationReadFile(path));
    if (r) setPending(r);
  };
  const install = async () => {
    if (!pending) return;
    const r = await attempt(ipc.installIntegration(pending.json), (i) => `Intégration « ${i.name} » installée.`);
    if (r) { setPending(null); await refresh(); }
  };
  const exportOne = async (i: IntegrationInfo) => {
    const path = await save({ defaultPath: `${i.name}.tgi`, filters: [{ name: "Intégration Tribal Grab", extensions: ["tgi"] }] });
    if (!path) return;
    await attempt(ipc.exportIntegrationFile(i.name, path), "Intégration exportée.");
  };
  const toggle = async (i: IntegrationInfo, enabled: boolean) => {
    setList((l) => l.map((x) => (x.name === i.name ? { ...x, enabled } : x)));
    await attempt(ipc.setIntegrationEnabled(i.name, enabled));
    await refresh();
  };

  return (
    <Page>
      <p className="mb-3 text-[12px] leading-relaxed text-muted-foreground">
        Une intégration est un petit script qui apprend à Tribal Grab à lire un site précis. N'installez que des fichiers dont vous connaissez l'origine : le code est affiché avant toute installation.
      </p>
      <div className="mb-4 flex items-center gap-2">
        <Button className={btnCls} onClick={() => setEditing({ info: NEW_INTEGRATION, isNew: true })}><FilePlus2 />Nouvelle intégration</Button>
        <Button variant="outline" className={btnCls} onClick={importFile}><Upload />Importer un fichier .tgi</Button>
      </div>

      <div className="rounded-lg border border-border bg-card">
        {list.length === 0 && <div className="px-4 py-8 text-center text-[12px] text-muted-foreground">Aucune intégration installée.</div>}
        {list.map((i) => (
          <div key={i.name} className="flex items-start gap-3 border-b border-border/60 px-4 py-3 last:border-b-0">
            <Switch checked={i.enabled} onCheckedChange={(v) => toggle(i, v)} className="mt-0.5" aria-label={`Activer ${i.name}`} />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="truncate text-[13px] font-medium">{i.name}</span>
                {i.requires_account && <Badge variant="secondary" className="text-[10px]">compte requis</Badge>}
              </div>
              <div className="num selectable truncate text-[11px] text-muted-foreground" title={i.url_regex}>{i.url_regex}</div>
              {i.comment && <div className="mt-0.5 text-[12px] leading-snug text-muted-foreground">{i.comment}</div>}
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button variant="ghost" size="xs" onClick={() => setEditing({ info: i, isNew: false })}><Pencil />Modifier</Button>
              <Button variant="ghost" size="xs" onClick={() => exportOne(i)}><Download />Exporter</Button>
              <Button variant="ghost" size="xs" className="text-destructive hover:text-destructive" onClick={() => setToDelete(i)}><Trash2 />Supprimer</Button>
            </div>
          </div>
        ))}
      </div>

      {editing && (
        <IntegrationEditor
          key={editing.isNew ? "new" : editing.info.name}
          info={editing.info}
          isNew={editing.isNew}
          onClose={() => setEditing(null)}
          onSaved={async () => { setEditing(null); await refresh(); }}
        />
      )}

      <Dialog open={pending !== null} onOpenChange={(o) => { if (!o) setPending(null); }}>
        <DialogContent className="max-h-[92vh] overflow-hidden sm:max-w-[680px]">
          <DialogHeader>
            <DialogTitle>Installer cette intégration ?</DialogTitle>
            <DialogDescription>Relisez le code ci-dessous : il s'exécutera sur votre Mac chaque fois qu'un lien correspondra.</DialogDescription>
          </DialogHeader>
          {pending && (
            <div className="min-w-0 space-y-2.5">
              <dl className="grid grid-cols-[88px_1fr] gap-x-3 gap-y-1.5 text-[12px]">
                <dt className="text-muted-foreground">Nom</dt>
                <dd className="selectable font-medium">{pending.info.name}</dd>
                <dt className="text-muted-foreground">Expression</dt>
                <dd className="num selectable break-all">{pending.info.url_regex}</dd>
                <dt className="text-muted-foreground">Commentaire</dt>
                <dd className="selectable">{pending.info.comment || "–"}</dd>
                <dt className="text-muted-foreground">Compte</dt>
                <dd>{pending.info.requires_account ? "Requis" : "Non requis"}</dd>
              </dl>
              <div className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">Code</div>
              <pre className="num selectable max-h-[280px] overflow-auto whitespace-pre rounded-md border border-border bg-background p-2.5 text-[12px] leading-relaxed">{pending.info.code}</pre>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" className={btnCls} onClick={() => setPending(null)}>Annuler</Button>
            <Button className={btnCls} onClick={install}>Installer</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={toDelete !== null} onOpenChange={(o) => { if (!o) setToDelete(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Supprimer l'intégration « {toDelete?.name} » ?</AlertDialogTitle>
            <AlertDialogDescription>Le script est supprimé. Exportez-le d'abord si vous voulez le conserver.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Annuler</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={async () => {
              if (!toDelete) return;
              await attempt(ipc.removeIntegration(toDelete.name), "Intégration supprimée.");
              setToDelete(null);
              await refresh();
            }}>Supprimer</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Page>
  );
}
