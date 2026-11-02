import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Input } from "@/components/ui/input";
import { SettingGroup, SettingRow } from "@/components/app/Setting";
import { cn } from "@/lib/utils";
import { fieldCls, Note, NumField, Page, PathField, ToggleRow, useS } from "./controls";

const schema = z.object({
  name_template: z.string().trim().min(1, "Le modèle ne peut pas être vide.").refine((v) => v.includes("{title}"), "Le modèle doit contenir {title}."),
});
type Form = z.infer<typeof schema>;

const VARS: [string, string][] = [
  ["{title}", "titre"], ["{id}", "identifiant"], ["{site}", "site"], ["{host}", "domaine"], ["{uploader}", "auteur"],
  ["{upload_date}", "date de mise en ligne"], ["{timestamp}", "horodatage"], ["{playlist}", "liste de lecture"],
  ["{playlist_index}", "position dans la liste"], ["{playlist_count}", "taille de la liste"], ["{resolution}", "résolution"],
  ["{ext}", "extension"], ["{original_name}", "nom d'origine"], ["{last_path}", "dernier segment de l'adresse"], ["{download_id}", "numéro du téléchargement"],
];
const SAMPLE: Record<string, string> = {
  title: "Ma vidéo", id: "dQw4w9WgXcQ", site: "YouTube", host: "youtube.com", uploader: "Ma chaîne", upload_date: "20260312",
  timestamp: "1773300000", playlist: "Ma liste", playlist_index: "03", playlist_count: "12", resolution: "1080p", ext: "mp4",
  original_name: "video", last_path: "watch", download_id: "42",
};
const preview = (tpl: string) => tpl.replace(/\{(\w+)\}/g, (m, k: string) => SAMPLE[k] ?? m) + ".mp4";

export default function Fichiers() {
  const { s, patch } = useS();
  const { register, watch, trigger, getValues, setValue, formState: { errors } } = useForm<Form>({
    resolver: zodResolver(schema),
    defaultValues: { name_template: s.name_template },
    mode: "onChange",
  });
  const current = watch("name_template");
  useEffect(() => {
    if (document.activeElement?.getAttribute("name") !== "name_template" && getValues("name_template") !== s.name_template) {
      setValue("name_template", s.name_template);
    }
  }, [s.name_template, getValues, setValue]);
  const field = register("name_template", {
    onChange: async () => {
      if (await trigger("name_template")) patch({ name_template: getValues("name_template").trim() });
    },
  });
  const err = errors.name_template?.message;

  return (
    <Page>
      <SettingGroup title="Destination">
        <SettingRow label="Dossier de téléchargement">
          <PathField directory value={s.download_dir} width={240} onChange={(v) => { if (v) patch({ download_dir: v }); }} />
        </SettingRow>
        <ToggleRow label="Choisir la destination à chaque téléchargement" checked={s.ask_destination} onChange={(v) => patch({ ask_destination: v })} />
        <SettingRow label="Espace libre minimal" hint="Refuse de démarrer si le disque a moins d'espace libre.">
          <NumField value={s.min_free_mb} min={0} max={10_000_000} suffix="Mo" width={96} onChange={(n) => patch({ min_free_mb: n })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Nom des fichiers">
        <div className="py-3">
          <div className="mb-1.5 text-[13px] font-medium">Modèle de nom</div>
          <Input
            {...field}
            spellCheck={false}
            autoCapitalize="off"
            aria-invalid={err ? true : undefined}
            className={cn(fieldCls, "num")}
          />
          {err
            ? <div className="mt-1.5 text-[12px] text-destructive">{err}</div>
            : <div className="mt-1.5 text-[12px] text-muted-foreground">Exemple : <span className="num selectable text-foreground">{preview(current)}</span></div>}
          <Note className="mt-1.5">Une barre oblique crée un sous-dossier, par exemple {"{site}/{title}"}.</Note>
        </div>
        <div className="border-t border-border/60 py-3">
          <div className="mb-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">Variables</div>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1">
            {VARS.map(([v, d]) => (
              <div key={v} className="flex items-baseline gap-2 text-[12px]">
                <code className="num w-[118px] shrink-0 text-foreground">{v}</code>
                <span className="truncate text-muted-foreground">{d}</span>
              </div>
            ))}
          </div>
        </div>
      </SettingGroup>
    </Page>
  );
}
