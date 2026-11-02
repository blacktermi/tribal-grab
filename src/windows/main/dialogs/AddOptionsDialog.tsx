import { useEffect } from "react";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ipc, errMsg } from "@/lib/ipc";
import { RESOLUTIONS } from "@/lib/format";
import type { AudioFormat, PostMode, Priority, SubMode } from "@/types";

const schema = z.object({
  text: z.string().min(1, "Collez au moins un lien."),
  kind: z.enum(["default", "mp4", "audio"]),
  resolution: z.string(),
  audio: z.enum(["M4a", "Mp3", "Original"]),
  subs: z.enum(["inherit", "None", "IfAvailable", "Required", "Only"]),
  dest: z.string(),
  priority: z.enum(["High", "Normal", "Low"]),
  when: z.string(),
});
type Form = z.infer<typeof schema>;

const links = (t: string) => [...new Set((t.match(/https?:\/\/[^\s<>"'`]+/gi) ?? []).map((u) => u.replace(/[.,;)\]!?]+$/, "")))];

export function AddOptionsDialog({ open: isOpen, initialText, onClose }: { open: boolean; initialText: string; onClose: () => void }) {
  const { register, handleSubmit, control, reset, setValue, watch, formState: { errors, isSubmitting } } = useForm<Form>({
    resolver: zodResolver(schema),
    defaultValues: { text: "", kind: "default", resolution: "inherit", audio: "M4a", subs: "inherit", dest: "", priority: "Normal", when: "" },
  });
  useEffect(() => { if (isOpen) reset({ text: initialText, kind: "default", resolution: "inherit", audio: "M4a", subs: "inherit", dest: "", priority: "Normal", when: "" }); }, [isOpen, initialText, reset]);
  const kind = watch("kind");

  async function onSubmit(v: Form) {
    const urls = links(v.text);
    if (urls.length === 0) { toast.error("Aucune adresse http ou https trouvée."); return; }
    const post_mode: PostMode = v.kind === "audio" ? "AudioOnly" : v.kind === "mp4" ? "Mp4" : "Default";
    const when = v.when ? Math.floor(new Date(v.when).getTime() / 1000) : null;
    try {
      for (const url of urls) {
        await ipc.add({
          url,
          priority: v.priority as Priority,
          scheduled_at: when,
          options: {
            post_mode,
            dest_dir: v.dest.trim() || null,
            max_resolution: v.resolution === "inherit" ? null : Number(v.resolution),
            audio_format: v.kind === "audio" ? (v.audio as AudioFormat) : null,
            sub_mode: v.subs === "inherit" ? null : (v.subs as SubMode),
          },
        });
      }
      toast.success(urls.length === 1 ? "Lien ajouté à la file" : `${urls.length} liens ajoutés à la file`);
      onClose();
    } catch (e) { toast.error(errMsg(e)); }
  }

  async function chooseDir() {
    const r = await open({ directory: true, title: "Dossier de destination" });
    if (typeof r === "string") setValue("dest", r);
  }
  const field = "grid grid-cols-[130px_1fr] items-center gap-3";
  return (
    <Dialog open={isOpen} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>Ajouter avec options</DialogTitle>
          <DialogDescription>Ces choix ne valent que pour les liens ajoutés ici. « Réglages » reprend vos préférences.</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit(onSubmit)} className="space-y-3">
          <div>
            <Textarea {...register("text")} rows={3} placeholder="Un lien par ligne" spellCheck={false} className="num text-[12px]" />
            {errors.text && <p className="mt-1 text-[12px] text-destructive">{errors.text.message}</p>}
          </div>
          <div className={field}>
            <Label>Contenu</Label>
            <Controller control={control} name="kind" render={({ field: f }) => (
              <Select value={f.value} onValueChange={f.onChange}><SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="default">Réglages (vidéo)</SelectItem><SelectItem value="mp4">Vidéo MP4</SelectItem><SelectItem value="audio">Audio seul</SelectItem></SelectContent></Select>
            )} />
          </div>
          {kind === "audio" ? (
            <div className={field}>
              <Label>Format audio</Label>
              <Controller control={control} name="audio" render={({ field: f }) => (
                <Select value={f.value} onValueChange={f.onChange}><SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent><SelectItem value="M4a">M4A</SelectItem><SelectItem value="Mp3">MP3</SelectItem><SelectItem value="Original">Format d'origine</SelectItem></SelectContent></Select>
              )} />
            </div>
          ) : (
            <div className={field}>
              <Label>Résolution max.</Label>
              <Controller control={control} name="resolution" render={({ field: f }) => (
                <Select value={f.value} onValueChange={f.onChange}><SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent><SelectItem value="inherit">Selon les réglages</SelectItem>{RESOLUTIONS.map((r) => <SelectItem key={r.label} value={r.value ? String(r.value) : "99999"}>{r.label}</SelectItem>)}</SelectContent></Select>
              )} />
            </div>
          )}
          <div className={field}>
            <Label>Sous-titres</Label>
            <Controller control={control} name="subs" render={({ field: f }) => (
              <Select value={f.value} onValueChange={f.onChange}><SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="inherit">Selon les réglages</SelectItem><SelectItem value="None">Aucun</SelectItem><SelectItem value="IfAvailable">Si disponibles</SelectItem>
                  <SelectItem value="Required">Obligatoires</SelectItem><SelectItem value="Only">Sous-titres seuls</SelectItem>
                </SelectContent></Select>
            )} />
          </div>
          <div className={field}>
            <Label>Destination</Label>
            <div className="flex gap-2"><Input {...register("dest")} placeholder="Dossier des réglages" className="num h-8 text-[12px]" /><Button type="button" variant="outline" size="sm" onClick={chooseDir}>Choisir…</Button></div>
          </div>
          <div className={field}>
            <Label>Priorité</Label>
            <Controller control={control} name="priority" render={({ field: f }) => (
              <Select value={f.value} onValueChange={f.onChange}><SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="High">Haute</SelectItem><SelectItem value="Normal">Normale</SelectItem><SelectItem value="Low">Basse</SelectItem></SelectContent></Select>
            )} />
          </div>
          <div className={field}>
            <Label>Démarrer le</Label>
            <Input type="datetime-local" {...register("when")} className="num h-8 text-[12px]" />
          </div>
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose}>Annuler</Button>
            <Button type="submit" disabled={isSubmitting}>Ajouter</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
