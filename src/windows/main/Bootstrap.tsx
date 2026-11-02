import { useEffect, useRef, useState } from "react";
import { AlertTriangle, Check, Copy, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ipc, errMsg } from "@/lib/ipc";
import { on, subscribe } from "@/app/events";
import type { ToolStatus } from "@/types";
import { toast } from "sonner";

/** Premier lancement : installe yt-dlp (publication officielle, SHA256 vérifié) avec un écran de progression. */
export function FirstRun({ onDone }: { onDone: () => void }) {
  const [phase, setPhase] = useState<"check" | "run" | "error">("check");
  const [error, setError] = useState("");
  const [elapsed, setElapsed] = useState(0);
  const started = useRef(false);

  async function run() {
    setPhase("run");
    setElapsed(0);
    const t0 = Date.now();
    const timer = setInterval(() => setElapsed(Math.round((Date.now() - t0) / 1000)), 500);
    try {
      await ipc.updateYtdlp();
      onDone();
    } catch (e) {
      setError(errMsg(e));
      setPhase("error");
    } finally {
      clearInterval(timer);
    }
  }

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    ipc.bootstrapStatus().then((s) => (s.needs_ytdlp ? run() : onDone())).catch(() => onDone());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (phase === "check") return null;
  const steps = [
    { label: "Téléchargement de la dernière version de yt-dlp", at: 0 },
    { label: "Vérification de l'empreinte SHA256 officielle", at: 8 },
    { label: "Première analyse par macOS (une seule fois)", at: 14 },
  ];
  const current = elapsed >= 14 ? 2 : elapsed >= 8 ? 1 : 0;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/95 backdrop-blur-sm">
      <div className="w-[420px]">
        <h1 className="text-[17px] font-semibold">Préparation de Tribal Grab</h1>
        <p className="mt-1 text-[13px] text-muted-foreground">yt-dlp est le moteur d'extraction pour plus de 1 800 sites. Il est installé une seule fois dans votre dossier de données.</p>
        {phase === "run" && (
          <>
            <ul className="mt-5 space-y-2.5">
              {steps.map((s, i) => (
                <li key={s.label} className="flex items-center gap-2.5 text-[13px]">
                  {i < current ? <Check className="size-4 text-primary" /> : i === current ? <Loader2 className="size-4 animate-spin text-primary" /> : <span className="size-4 rounded-full border border-border" />}
                  <span className={i > current ? "text-muted-foreground" : ""}>{s.label}</span>
                </li>
              ))}
            </ul>
            <div className="mt-5 h-1 overflow-hidden rounded-full bg-muted"><div className="relative h-full"><div className="absolute inset-y-0 w-1/3 animate-[indet_1.4s_ease-in-out_infinite] rounded-full bg-primary" /></div></div>
            <div className="num mt-2 text-[12px] text-muted-foreground">{elapsed} s · environ 30 s au total</div>
          </>
        )}
        {phase === "error" && (
          <div className="mt-5">
            <div className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 p-3 text-[13px]"><AlertTriangle className="mt-0.5 size-4 shrink-0 text-destructive" /><span className="selectable">{error}</span></div>
            <div className="mt-4 flex gap-2"><Button onClick={run}>Réessayer</Button><Button variant="ghost" onClick={onDone}>Continuer sans yt-dlp</Button></div>
          </div>
        )}
      </div>
    </div>
  );
}

function CopyCmd({ cmd }: { cmd: string }) {
  return (
    <button className="num inline-flex items-center gap-1.5 rounded border border-border bg-muted px-1.5 py-0.5 text-[11.5px] text-foreground hover:bg-accent" onClick={() => navigator.clipboard.writeText(cmd).then(() => toast.success("Commande copiée"))}>
      {cmd}<Copy className="size-3" />
    </button>
  );
}

/** Bandeau : ce qui manque (ffmpeg, runtime JavaScript) et la commande pour l'installer. */
export function ToolsBanner() {
  const [tools, setTools] = useState<ToolStatus[] | null>(null);
  const [hidden, setHidden] = useState(false);
  const load = () => ipc.tools().then(setTools).catch(() => {});
  useEffect(() => {
    load();
    return subscribe(on<unknown>("grab://event", (e) => e === "ToolsChanged" && load()));
  }, []);
  if (!tools || hidden) return null;
  const has = (n: string) => !!tools.find((t) => t.name === n)?.path;
  const missing: { label: string; cmd: string }[] = [];
  if (!has("ffmpeg") || !has("ffprobe")) missing.push({ label: "ffmpeg (fusion, conversion, audio)", cmd: "brew install ffmpeg" });
  if (!has("deno") && !has("node")) missing.push({ label: "un runtime JavaScript (YouTube et sites protégés)", cmd: "brew install deno" });
  if (!has("yt-dlp")) missing.push({ label: "yt-dlp (mise à jour dans Préférences > Avancé)", cmd: "" });
  if (missing.length === 0) return null;
  return (
    <div className="flex shrink-0 items-start gap-3 border-b border-border bg-primary/10 px-4 py-2.5 text-[12.5px]">
      <AlertTriangle className="mt-0.5 size-4 shrink-0 text-primary" />
      <div className="min-w-0 flex-1 space-y-1">
        <div className="font-medium">Il manque des outils pour certains téléchargements</div>
        {missing.map((m) => (
          <div key={m.label} className="flex flex-wrap items-center gap-2 text-muted-foreground">
            <span>{m.label}</span>{m.cmd && <>: <CopyCmd cmd={m.cmd} /></>}
          </div>
        ))}
      </div>
      <Button size="xs" variant="ghost" onClick={load}>Revérifier</Button>
      <Button size="xs" variant="ghost" onClick={() => setHidden(true)}>Masquer</Button>
    </div>
  );
}
