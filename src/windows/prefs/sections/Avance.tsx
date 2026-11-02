import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Copy, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { SettingGroup, SettingRow } from "@/components/app/Setting";
import { attempt } from "@/components/app/errors";
import { onGrabEvent, subscribe } from "@/app/events";
import { ipc } from "@/lib/ipc";
import type { AppInfo, ToolStatus, UpdateInfo } from "@/types";
import { btnCls, NumField, Page, ToggleRow, useS } from "./controls";

const LABELS: Record<string, string> = { "yt-dlp": "yt-dlp", ffmpeg: "ffmpeg", ffprobe: "ffprobe", deno: "deno", node: "node" };
const INSTALL: Record<string, string> = {
  ffmpeg: "brew install ffmpeg", ffprobe: "brew install ffmpeg", deno: "brew install deno", node: "brew install node",
};
const SOURCE: Record<string, string> = {
  custom: "Choisi", manual: "Choisi", bundled: "Intégré", managed: "Géré par l'application", system: "Système", path: "Système", homebrew: "Homebrew",
};

async function copy(text: string) {
  try { await navigator.clipboard.writeText(text); toast.success("Copié."); } catch { toast.error("Copie impossible."); }
}

function Tools() {
  const [tools, setTools] = useState<ToolStatus[]>([]);
  const load = useCallback(async () => {
    const t = await attempt(ipc.tools());
    if (t) setTools(t);
  }, []);
  useEffect(() => {
    void load();
    return subscribe(onGrabEvent((e) => { if (e === "ToolsChanged") void load(); }));
  }, [load]);

  const choose = async (name: string) => {
    const r = await open({ multiple: false, directory: false, title: `Choisir ${name}` });
    if (typeof r !== "string") return;
    await attempt(ipc.setToolPath(name, r));
    await load();
  };
  const reset = async (name: string) => {
    await attempt(ipc.setToolPath(name, null));
    await load();
  };

  return (
    <SettingGroup title="Outils">
      {tools.length === 0 && <div className="py-4 text-[12px] text-muted-foreground">Lecture des outils…</div>}
      {tools.map((t) => {
        const cmd = INSTALL[t.name];
        return (
          <div key={t.name} className="border-b border-border/60 py-2.5 last:border-b-0">
            <div className="flex items-center gap-3">
              <div className="w-[70px] shrink-0 text-[13px] font-medium">{LABELS[t.name] ?? t.name}</div>
              <div className="num w-[80px] shrink-0 truncate text-[12px] text-muted-foreground" title={t.version ?? undefined}>{t.version ?? "absent"}</div>
              <div className="min-w-0 flex-1">
                <div className="num selectable truncate text-[11px] text-muted-foreground" title={t.path ?? undefined}>{t.path ?? "–"}</div>
                {t.source && <div className="text-[11px] text-muted-foreground">{SOURCE[t.source] ?? t.source}</div>}
              </div>
              <Button variant="outline" className={btnCls} onClick={() => choose(t.name)}>Choisir…</Button>
              <Button variant="ghost" className={btnCls} onClick={() => reset(t.name)}>Rétablir</Button>
            </div>
            {!t.path && cmd && (
              <div className="mt-2 flex items-center gap-2 rounded-md border border-border bg-background px-2.5 py-1.5">
                <span className="text-[12px] text-muted-foreground">Installer :</span>
                <code className="num selectable flex-1 text-[12px]">{cmd}</code>
                <Button variant="ghost" size="icon-xs" title="Copier la commande" aria-label="Copier la commande" onClick={() => copy(cmd)}><Copy /></Button>
              </div>
            )}
          </div>
        );
      })}
    </SettingGroup>
  );
}

function YtdlpUpdate() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [current, setCurrent] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const timer = useRef<ReturnType<typeof setInterval> | undefined>(undefined);

  useEffect(() => {
    void ipc.tools().then((t) => setCurrent(t.find((x) => x.name === "yt-dlp")?.version ?? null)).catch(() => {});
    return () => clearInterval(timer.current);
  }, []);

  const check = async () => {
    setChecking(true);
    const r = await attempt(ipc.checkYtdlpUpdate());
    setChecking(false);
    if (r) { setInfo(r); setCurrent(r.current); }
  };
  const update = async () => {
    setUpdating(true);
    setElapsed(0);
    timer.current = setInterval(() => setElapsed((n) => n + 1), 1000);
    const r = await attempt(ipc.updateYtdlp(), (m) => (m && m.trim() ? m : "yt-dlp est à jour."));
    clearInterval(timer.current);
    setUpdating(false);
    if (r !== undefined) {
      setInfo(null);
      const t = await attempt(ipc.tools());
      setCurrent(t?.find((x) => x.name === "yt-dlp")?.version ?? null);
    }
  };

  return (
    <SettingGroup title="Mise à jour de yt-dlp">
      <SettingRow label="Version actuelle" hint={info ? (info.available ? <>Version <span className="num text-primary">{info.latest}</span> disponible.</> : "Vous avez la dernière version.") : undefined}>
        <span className="num text-[13px]">{current ?? info?.current ?? "absent"}</span>
      </SettingRow>
      <div className="flex items-center gap-2 py-3">
        <Button variant="outline" className={btnCls} disabled={checking || updating} onClick={check}>
          {checking && <Loader2 className="animate-spin" />}Rechercher une mise à jour
        </Button>
        <Button className={btnCls} disabled={updating || checking} onClick={update}>
          {updating && <Loader2 className="animate-spin" />}Mettre à jour
        </Button>
        {updating && <span className="num text-[12px] text-muted-foreground">{elapsed} s · environ 30 s</span>}
      </div>
    </SettingGroup>
  );
}

export default function Avance() {
  const { s, patch } = useS();
  const [app, setApp] = useState<AppInfo | null>(null);
  useEffect(() => {
    void attempt(ipc.appInfo()).then((a) => { if (a) setApp(a); });
  }, []);
  return (
    <Page>
      <Tools />
      <YtdlpUpdate />

      <SettingGroup title="Directs">
        <ToggleRow label="Enregistrer les directs en MPEG-TS" hint="Le fichier reste lisible même si l'enregistrement est interrompu." checked={s.live_mpegts} onChange={(v) => patch({ live_mpegts: v })} />
        <SettingRow label="Espace libre minimal pendant un direct" hint="L'enregistrement s'arrête proprement sous ce seuil.">
          <NumField value={s.live_min_free_mb} min={0} max={10_000_000} suffix="Mo" width={96} onChange={(n) => patch({ live_min_free_mb: n })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Dossier de données">
        <SettingRow label="Emplacement">
          <div className="flex items-center gap-2">
            <span className="num selectable max-w-[280px] truncate text-[12px]" title={app?.data_dir}>{app?.data_dir ?? "…"}</span>
            {app && <Button variant="ghost" size="xs" onClick={() => attempt(ipc.revealInFinder(app.data_dir))}>Afficher</Button>}
          </div>
        </SettingRow>
        <SettingRow label="Version de l'application">
          <span className="num text-[13px]">{app?.version ?? "…"}</span>
        </SettingRow>
      </SettingGroup>
    </Page>
  );
}
