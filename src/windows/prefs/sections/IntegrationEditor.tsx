import { useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Loader2, Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { attempt } from "@/components/app/errors";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { duration } from "@/lib/format";
import type { IntegrationInfo, IntegrationTest } from "@/types";
import { btnCls, fieldCls } from "./controls";

const schema = z.object({
  name: z.string().trim().min(1, "Le nom est obligatoire."),
  url_regex: z.string().trim().min(1, "L'expression est obligatoire.").refine((v) => {
    try { new RegExp(v); return true; } catch { return false; }
  }, "Expression régulière invalide."),
  timeout_secs: z.number("Nombre attendu.").int("Nombre entier attendu.").min(1, "Au moins 1 seconde.").max(120, "120 secondes au maximum."),
  requires_account: z.boolean(),
  example_url: z.string().trim(),
  comment: z.string().trim(),
  code: z.string().min(1, "Le code est obligatoire."),
});
type Form = z.infer<typeof schema>;

function Err({ m }: { m?: string }) {
  return m ? <div className="mt-1 text-[12px] text-destructive">{m}</div> : null;
}

/** Les textes distants (titres, journaux, URL) sont toujours rendus comme du texte React. */
function TestResult({ r }: { r: IntegrationTest }) {
  const res = r.result;
  return (
    <div className="space-y-2.5">
      {r.logs.length > 0 && (
        <div>
          <div className="mb-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">Journal</div>
          <div className="num selectable max-h-[120px] overflow-auto rounded-md border border-border bg-background p-2 text-[12px] leading-relaxed">
            {r.logs.map((l, i) => <div key={i} className="whitespace-pre-wrap break-all">{l}</div>)}
          </div>
        </div>
      )}
      {r.error && (
        <div className="num selectable whitespace-pre-wrap break-words rounded-md border border-destructive/50 bg-destructive/10 p-2 text-[12px] text-destructive">{r.error}</div>
      )}
      {res && (
        <div>
          <div className="mb-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">Résultat</div>
          <div className="selectable space-y-1.5 rounded-md border border-border bg-background p-2.5 text-[12px]">
            <div className="text-[13px] font-medium">{res.title || "Sans titre"}</div>
            <div className="num text-muted-foreground">
              {res.id}{res.duration != null ? ` · ${duration(res.duration)}` : ""}{res.thumbnail ? " · miniature" : ""}
            </div>
            <div className="text-muted-foreground">{res.formats.length} format{res.formats.length > 1 ? "s" : ""}</div>
            <ul className="space-y-0.5">
              {res.formats.map((f, i) => (
                <li key={i} className="flex gap-2">
                  <span className="num w-[130px] shrink-0">{f.height ? `${f.height}p` : "?"} · {f.ext} · {f.protocol}</span>
                  <span className="num min-w-0 flex-1 truncate text-muted-foreground" title={f.url}>{f.url}</span>
                </li>
              ))}
            </ul>
            {res.subtitles.length > 0 && (
              <>
                <div className="pt-1 text-muted-foreground">{res.subtitles.length} sous-titre{res.subtitles.length > 1 ? "s" : ""}</div>
                <ul className="space-y-0.5">
                  {res.subtitles.map((t, i) => (
                    <li key={i} className="flex gap-2">
                      <span className="num w-[130px] shrink-0">{t.lang} · {t.ext}</span>
                      <span className="num min-w-0 flex-1 truncate text-muted-foreground" title={t.url}>{t.url}</span>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </div>
        </div>
      )}
      {!res && !r.error && <div className="text-[12px] text-muted-foreground">Le script n'a renvoyé aucun résultat.</div>}
    </div>
  );
}

export default function IntegrationEditor({
  info, isNew, onClose, onSaved,
}: { info: IntegrationInfo; isNew: boolean; onClose: () => void; onSaved: () => void | Promise<void> }) {
  const { register, handleSubmit, watch, setValue, getValues, formState: { errors, isSubmitting } } = useForm<Form>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: info.name, url_regex: info.url_regex, timeout_secs: info.timeout_secs, requires_account: info.requires_account,
      example_url: info.example_url ?? "", comment: info.comment ?? "", code: info.code,
    },
  });
  const [testUrl, setTestUrl] = useState(info.example_url ?? "");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<IntegrationTest | null>(null);

  const build = (v: Form): IntegrationInfo => ({
    name: v.name.trim(), url_regex: v.url_regex.trim(), enabled: info.enabled, timeout_secs: v.timeout_secs,
    requires_account: v.requires_account, example_url: v.example_url.trim() || null, comment: v.comment.trim() || null, code: v.code,
  });

  const submit = handleSubmit(async (v) => {
    const r = await attempt(ipc.saveIntegration(build(v)), "Intégration enregistrée.");
    if (r) await onSaved();
  });

  const run = async () => {
    const v = getValues();
    const timeout = Number.isFinite(v.timeout_secs) ? Math.min(120, Math.max(1, v.timeout_secs)) : 30;
    setRunning(true);
    setResult(null);
    const r = await attempt(ipc.testIntegration(build({ ...v, timeout_secs: timeout }), testUrl.trim()));
    setRunning(false);
    if (r) setResult(r);
  };

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onClose(); }}>
      <DialogContent className="max-h-[94vh] gap-3 overflow-y-auto sm:max-w-[780px]">
        <DialogHeader>
          <DialogTitle>{isNew ? "Nouvelle intégration" : `Modifier « ${info.name} »`}</DialogTitle>
          <DialogDescription>Le script reçoit l'adresse dans la variable url et rend son résultat avec grab.result.</DialogDescription>
        </DialogHeader>

        <form onSubmit={submit} className="space-y-3">
          <div className="grid grid-cols-[1fr_1fr] gap-3">
            <div>
              <Label className="mb-1 text-[12px]">Nom</Label>
              <Input {...register("name")} readOnly={!isNew} spellCheck={false} aria-invalid={errors.name ? true : undefined} className={fieldCls} />
              <Err m={errors.name?.message} />
            </div>
            <div>
              <Label className="mb-1 text-[12px]">Expression des adresses</Label>
              <Input {...register("url_regex")} spellCheck={false} autoCapitalize="off" aria-invalid={errors.url_regex ? true : undefined} className={cn(fieldCls, "num")} />
              <Err m={errors.url_regex?.message} />
            </div>
            <div>
              <Label className="mb-1 text-[12px]">Adresse d'exemple</Label>
              <Input {...register("example_url", { onChange: (e) => setTestUrl((e.target as HTMLInputElement).value) })} spellCheck={false} className={cn(fieldCls, "num")} />
            </div>
            <div className="grid grid-cols-[110px_1fr] gap-3">
              <div>
                <Label className="mb-1 text-[12px]">Délai (s)</Label>
                <Input type="number" min={1} max={120} {...register("timeout_secs", { valueAsNumber: true })} aria-invalid={errors.timeout_secs ? true : undefined} className={cn(fieldCls, "num")} />
              </div>
              <div>
                <Label className="mb-1 text-[12px]">Compte requis</Label>
                <div className="flex h-[30px] items-center">
                  <Switch checked={watch("requires_account")} onCheckedChange={(v) => setValue("requires_account", v)} aria-label="Compte requis" />
                </div>
              </div>
            </div>
          </div>
          <Err m={errors.timeout_secs?.message} />
          <div>
            <Label className="mb-1 text-[12px]">Commentaire</Label>
            <Input {...register("comment")} className={fieldCls} />
          </div>
          <div>
            <Label className="mb-1 text-[12px]">Code (JavaScript)</Label>
            <Textarea {...register("code")} rows={14} spellCheck={false} aria-invalid={errors.code ? true : undefined}
              className="num min-h-[240px] resize-y whitespace-pre text-[12px] leading-relaxed md:text-[12px]" />
            <Err m={errors.code?.message} />
          </div>

          <div className="rounded-lg border border-border p-3">
            <div className="mb-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">Console de test</div>
            <div className="mb-2 flex items-center gap-2">
              <Input value={testUrl} onChange={(e) => setTestUrl(e.target.value)} placeholder="https://…" spellCheck={false} className={cn(fieldCls, "num flex-1")} />
              <Button type="button" variant="outline" className={btnCls} disabled={running || testUrl.trim() === ""} onClick={run}>
                {running ? <Loader2 className="animate-spin" /> : <Play />}Exécuter
              </Button>
            </div>
            {result && <TestResult r={result} />}
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" className={btnCls} onClick={onClose}>Annuler</Button>
            <Button type="submit" className={btnCls} disabled={isSubmitting}>Enregistrer</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
