import { useState } from "react";
import { Copy, FolderOpen } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { SettingGroup } from "@/components/app/Setting";
import { attempt } from "@/components/app/errors";
import { ipc } from "@/lib/ipc";
import { btnCls, Page } from "./controls";

function Code({ children }: { children: string }) {
  return (
    <div className="flex items-center gap-2 rounded-md border border-border bg-background px-2.5 py-1.5">
      <code className="num selectable min-w-0 flex-1 overflow-x-auto whitespace-nowrap text-[12px]">{children}</code>
      <Button variant="ghost" size="icon-xs" title="Copier" aria-label="Copier"
        onClick={() => { void navigator.clipboard.writeText(children).then(() => toast.success("Copié.")); }}>
        <Copy />
      </Button>
    </div>
  );
}

export default function Extensions() {
  const [path, setPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const prepare = async () => {
    setBusy(true);
    const r = await attempt(ipc.extensionInstall());
    setBusy(false);
    if (r) setPath(r);
  };
  const steps: string[] = [
    "Ouvrez chrome://extensions dans Chrome, Brave, Edge ou Arc.",
    "Activez le « Mode développeur » en haut à droite.",
    "Cliquez sur « Charger l'extension non empaquetée ».",
    "Choisissez le dossier affiché ci-dessus.",
  ];
  return (
    <Page>
      <SettingGroup title="Extension pour navigateurs Chromium">
        <div className="space-y-3 py-3">
          <p className="text-[12px] leading-relaxed text-muted-foreground">
            Le bouton de l'extension envoie l'onglet courant à Tribal Grab par l'adresse{" "}
            <code className="num text-foreground">tribalgrab://add?url=…</code>.
            L'option « Fermer l'onglet après l'envoi » se règle dans les options de l'extension.
          </p>
          <div className="flex items-center gap-2">
            <Button className={btnCls} disabled={busy} onClick={prepare}><FolderOpen />Préparer l'extension</Button>
            <span className="text-[12px] text-muted-foreground">Copie l'extension dans le dossier de données et l'affiche dans le Finder.</span>
          </div>
          {path && <Code>{path}</Code>}
          <ol className="space-y-1.5 pt-1">
            {steps.map((t, i) => (
              <li key={i} className="flex gap-2.5 text-[13px]">
                <span className="num flex size-5 shrink-0 items-center justify-center rounded-full bg-secondary text-[11px] text-muted-foreground">{i + 1}</span>
                <span className="pt-px">{t}</span>
              </li>
            ))}
          </ol>
        </div>
      </SettingGroup>

      <SettingGroup title="Depuis le Terminal et Raccourcis">
        <div className="space-y-3 py-3">
          <div className="space-y-1.5">
            <div className="text-[12px] text-muted-foreground">Ajouter un lien avec l'adresse personnalisée :</div>
            <Code>{'open "tribalgrab://add?url=https://…"'}</Code>
          </div>
          <div className="space-y-1.5">
            <div className="text-[12px] text-muted-foreground">Ligne de commande, utilisable dans Raccourcis avec l'action « Exécuter un script shell » :</div>
            <Code>tribal-grab get https://…</Code>
          </div>
        </div>
      </SettingGroup>
    </Page>
  );
}
