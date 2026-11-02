import { useEffect, useRef, useState } from "react";
import { SettingGroup, SettingRow } from "@/components/app/Setting";
import { Textarea } from "@/components/ui/textarea";
import type { PostMode, ScriptWhen } from "@/types";
import { Page, SelectField, TextField, ToggleRow, useS } from "./controls";

const MODES: readonly (readonly [PostMode, string])[] = [
  ["None", "Aucun"], ["Mp4", "Convertir en MP4"], ["AudioOnly", "Audio seul"], ["SendToApp", "Envoyer à une application"], ["Custom", "Script personnalisé"],
];
const WHEN: readonly (readonly [ScriptWhen, string])[] = [["PerFile", "Pour chaque fichier"], ["EndOfQueue", "En fin de file"]];

function ScriptBox({ value, onChange }: { value: string | null; onChange: (v: string | null) => void }) {
  const [text, setText] = useState(value ?? "");
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(value ?? "");
  }, [value]);
  return (
    <Textarea
      value={text}
      rows={6}
      spellCheck={false}
      placeholder={'# zsh\nmv "$TG_FILE" ~/Movies/'}
      className="num min-h-[120px] w-full resize-y text-[12px] leading-relaxed md:text-[12px]"
      onFocus={() => { focused.current = true; }}
      onBlur={() => { focused.current = false; }}
      onChange={(e) => {
        setText(e.target.value);
        onChange(e.target.value.trim() === "" ? null : e.target.value);
      }}
    />
  );
}

export default function Post() {
  const { s, patch } = useS();
  // « Default » n'a pas de sens ici : on l'affiche comme « Aucun ».
  const mode: PostMode = s.post_mode === "Default" ? "None" : s.post_mode;
  return (
    <Page>
      <SettingGroup title="Après le téléchargement">
        <SettingRow label="Traitement" hint="Appliqué à chaque fichier terminé.">
          <SelectField value={mode} options={MODES} onChange={(v) => patch({ post_mode: v })} />
        </SettingRow>
        <ToggleRow label="Ajouter à Musique" hint="Importe le fichier dans la médiathèque Musique." checked={s.add_to_music} onChange={(v) => patch({ add_to_music: v })} />
        <SettingRow label="Envoyer à l'application" hint="Nom de l'application, par exemple VLC.">
          <TextField value={s.send_to_app} placeholder="VLC" width={180} onChange={(v) => patch({ send_to_app: v })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Conversion et métadonnées">
        <ToggleRow label="Réencoder la vidéo" hint="Plus lent, mais garantit un fichier lisible partout." checked={s.reencode} onChange={(v) => patch({ reencode: v })} />
        <ToggleRow label="Garder aussi l'original après une conversion" hint="Quand une vidéo doit être convertie pour QuickTime, le flux d'origine reste à côté, sans perte, dans un fichier .mkv à ouvrir avec VLC." checked={s.keep_source_on_convert} onChange={(v) => patch({ keep_source_on_convert: v })} />
        <ToggleRow label="Conserver le fichier d'origine" checked={s.keep_original} onChange={(v) => patch({ keep_original: v })} />
        <ToggleRow label="Intégrer les métadonnées" checked={s.embed_metadata} onChange={(v) => patch({ embed_metadata: v })} />
        <ToggleRow label="Intégrer la miniature au fichier" checked={s.embed_thumbnail} onChange={(v) => patch({ embed_thumbnail: v })} />
        <ToggleRow label="Enregistrer la miniature à côté" checked={s.save_thumbnail} onChange={(v) => patch({ save_thumbnail: v })} />
        <ToggleRow label="Enregistrer les informations (JSON)" checked={s.save_json} onChange={(v) => patch({ save_json: v })} />
      </SettingGroup>

      <SettingGroup title="Script personnalisé">
        <SettingRow label="Exécution">
          <SelectField value={s.script_when} options={WHEN} onChange={(v) => patch({ script_when: v })} />
        </SettingRow>
        <div className="py-3">
          <div className="mb-1.5 text-[13px] font-medium">Script (zsh)</div>
          <ScriptBox value={s.script} onChange={(v) => patch({ script: v })} />
          <div className="mt-1.5 text-[12px] leading-snug text-muted-foreground">
            Variables disponibles : <code className="num text-foreground">TG_FILE</code>, <code className="num text-foreground">TG_TITLE</code>,{" "}
            <code className="num text-foreground">TG_URL</code>, <code className="num text-foreground">TG_SITE</code>, <code className="num text-foreground">TG_PLAYLIST</code>.
            Le script ne s'exécute que si le traitement « Script personnalisé » est choisi.
          </div>
        </div>
      </SettingGroup>
    </Page>
  );
}
