import { SettingGroup, SettingRow } from "@/components/app/Setting";
import { RESOLUTIONS } from "@/lib/format";
import type { AudioFormat, Container } from "@/types";
import { ListField, Page, SelectField, ToggleRow, useS } from "./controls";

const CONTAINERS: readonly (readonly [Container, string])[] = [
  ["PreferMp4", "Préférer le MP4"], ["ForceMp4", "Forcer le MP4"], ["Original", "Format d'origine"],
];
const AUDIO: readonly (readonly ["none" | AudioFormat, string])[] = [
  ["none", "Vidéo"], ["M4a", "Audio seul : M4A"], ["Mp3", "Audio seul : MP3"], ["Original", "Audio seul : format d'origine"],
];
const RES: readonly (readonly [string, string])[] = RESOLUTIONS.map((r) => [String(r.value), r.label] as const);

export default function Qualite() {
  const { s, patch } = useS();
  return (
    <Page>
      <SettingGroup title="Vidéo">
        <SettingRow label="Résolution maximale" hint="La meilleure qualité disponible sous cette limite est choisie.">
          <SelectField
            width={240}
            value={String(s.max_resolution)}
            options={RES}
            onChange={(v) => patch({ max_resolution: v === "null" ? null : Number(v) })}
          />
        </SettingRow>
        <ToggleRow label="Demander la qualité à chaque vidéo" checked={s.quality_prompt} onChange={(v) => patch({ quality_prompt: v })} />
        <SettingRow label="Conteneur">
          <SelectField value={s.container} options={CONTAINERS} onChange={(v) => patch({ container: v })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Audio">
        <SettingRow label="Type de téléchargement" hint="L'audio seul ignore la vidéo.">
          <SelectField
            width={240}
            value={s.audio_format ?? "none"}
            options={AUDIO}
            onChange={(v) => patch({ audio_format: v === "none" ? null : v })}
          />
        </SettingRow>
        <SettingRow label="Langues audio préférées" hint="Codes de langue séparés par des virgules, par ordre de préférence (fr, en, ja).">
          <ListField value={s.audio_languages} placeholder="fr, en" width={200} onChange={(v) => patch({ audio_languages: v })} />
        </SettingRow>
      </SettingGroup>
    </Page>
  );
}
