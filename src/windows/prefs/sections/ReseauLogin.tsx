import { useCallback, useEffect, useMemo, useState } from "react";
import { Cookie, Database, HardDrive, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { SettingGroup, SettingRow } from "@/components/app/Setting";
import { attempt } from "@/components/app/errors";
import { ipc } from "@/lib/ipc";
import { dateTime } from "@/lib/format";
import type { LoginStatus, SiteData } from "@/types";
import { btnCls, fieldCls } from "./controls";

type Kind = "cookies" | "local" | "session";

export default function ReseauLogin() {
  const [status, setStatus] = useState<LoginStatus | null>(null);
  const [sites, setSites] = useState<SiteData[]>([]);
  const [filter, setFilter] = useState("");
  const [confirmAll, setConfirmAll] = useState(false);

  const refresh = useCallback(async () => {
    const [st, list] = await Promise.all([
      attempt(ipc.loginStatus()),
      attempt(ipc.siteDataList()),
    ]);
    if (st) setStatus(st);
    if (list) setSites(list);
  }, []);

  useEffect(() => {
    void refresh();
    const onFocus = () => { void refresh(); };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q ? sites.filter((x) => x.domain.toLowerCase().includes(q)) : sites;
  }, [sites, filter]);

  const clear = async (domain: string, kinds: Kind[]) => {
    await attempt(ipc.siteDataClear(domain, kinds), "Données effacées.");
    await refresh();
  };

  return (
    <>
      <SettingGroup title="Connexion aux sites">
        <SettingRow
          label="Session du navigateur intégré"
          hint={status
            ? <span className="num">{status.cookies} cookie{status.cookies > 1 ? "s" : ""}{status.updated_at ? ` · mis à jour le ${dateTime(status.updated_at)}` : ""}</span>
            : "Lecture en cours…"}
        >
          <span className={status?.active ? "text-[12px] font-medium text-primary" : "text-[12px] text-muted-foreground"}>
            {status?.active ? "Utilisée" : "Inactive"}
          </span>
        </SettingRow>
        <div className="flex flex-wrap items-center gap-2 py-3">
          <Button variant="outline" className={btnCls} onClick={() => attempt(ipc.openWindow("browser", "login"))}>
            Ouvrir le navigateur de connexion
          </Button>
          <Button variant="outline" className={btnCls}
            onClick={async () => { await attempt(ipc.loginSave(), (n) => `${n} cookie${n > 1 ? "s" : ""} enregistré${n > 1 ? "s" : ""}.`); await refresh(); }}>
            Enregistrer les cookies maintenant
          </Button>
          <Button variant="ghost" className={btnCls} disabled={!status?.active}
            onClick={async () => { await attempt(ipc.loginDisable(), "Session désactivée."); await refresh(); }}>
            Ne plus utiliser
          </Button>
        </div>
      </SettingGroup>

      <SettingGroup title="Données par site">
        <div className="flex items-center gap-2 border-b border-border/60 py-2.5">
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filtrer les domaines" className={`${fieldCls} pl-8`} />
          </div>
          <Button variant="outline" className={btnCls} disabled={sites.length === 0} onClick={() => setConfirmAll(true)}>
            Tout effacer
          </Button>
        </div>
        {shown.length === 0 ? (
          <div className="py-6 text-center text-[12px] text-muted-foreground">
            {sites.length === 0 ? "Aucune donnée de site enregistrée." : "Aucun domaine ne correspond."}
          </div>
        ) : (
          <ul className="max-h-[260px] overflow-y-auto">
            {shown.map((x) => (
              <li key={x.domain} className="flex items-center gap-2 border-b border-border/60 py-1.5 last:border-b-0">
                <span className="num selectable min-w-0 flex-1 truncate text-[12px]" title={x.domain}>{x.domain}</span>
                <span className="num w-16 shrink-0 text-right text-[12px] text-muted-foreground">{x.cookies} cookie{x.cookies > 1 ? "s" : ""}</span>
                <Button variant="ghost" size="xs" title="Effacer les cookies" onClick={() => clear(x.domain, ["cookies"])}><Cookie />Cookies</Button>
                <Button variant="ghost" size="xs" title="Effacer le stockage local" onClick={() => clear(x.domain, ["local"])}><HardDrive />Local</Button>
                <Button variant="ghost" size="xs" title="Effacer le stockage de session" onClick={() => clear(x.domain, ["session"])}><Database />Session</Button>
              </li>
            ))}
          </ul>
        )}
      </SettingGroup>

      <AlertDialog open={confirmAll} onOpenChange={setConfirmAll}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Effacer les données de tous les sites ?</AlertDialogTitle>
            <AlertDialogDescription>Cookies, stockage local et stockage de session seront supprimés. Vous serez déconnecté des sites.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Annuler</AlertDialogCancel>
            <AlertDialogAction variant="destructive"
              onClick={async () => { await attempt(ipc.siteDataClearAll(), "Toutes les données ont été effacées."); await refresh(); }}>
              Tout effacer
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
