import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import type { IntegrationInfo } from "@/types";

/** Un fichier .tgi contient du code : rien ne s'installe sans cette confirmation. */
export function TgiConfirmDialog({ data, onClose }: { data: { json: string; info: IntegrationInfo } | null; onClose: () => void }) {
  const info = data?.info;
  return (
    <Dialog open={!!data} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Installer cette intégration ?</DialogTitle>
          <DialogDescription>Le code s'exécute dans un bac à sable sans accès au disque ni aux processus, mais il peut interroger le réseau. N'installez que des fichiers de confiance.</DialogDescription>
        </DialogHeader>
        {info && (
          <div className="space-y-2 text-[12.5px]">
            <div className="flex items-center gap-2"><span className="selectable text-[14px] font-semibold">{info.name}</span>{info.requires_account && <Badge variant="secondary">compte requis</Badge>}</div>
            <div className="text-muted-foreground">Liens concernés : <code className="num selectable text-foreground">{info.url_regex}</code></div>
            {info.comment && <div className="selectable text-muted-foreground">{info.comment}</div>}
            <pre className="selectable num max-h-64 overflow-auto whitespace-pre-wrap rounded-md border border-border bg-muted p-3 text-[11.5px]">{info.code}</pre>
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>Annuler</Button>
          <Button onClick={async () => { if (data && (await attempt(ipc.installIntegration(data.json), "Intégration installée")) !== undefined) onClose(); }}>Installer</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
