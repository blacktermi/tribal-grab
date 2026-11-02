import { SettingGroup, SettingRow } from "@/components/app/Setting";
import type { SubMode, SubOut } from "@/types";
import { ListField, Page, SelectField, TextField, ToggleRow, useS } from "./controls";

const MODES: readonly (readonly [SubMode, string])[] = [
  ["None", "Aucun"], ["IfAvailable", "Si disponibles"], ["Required", "Obligatoires"], ["Only", "Sous-titres seuls"],
];
const OUT: readonly (readonly [SubOut, string])[] = [["Srt", "SRT"], ["Vtt", "VTT"], ["Embed", "Intégrés au MP4 ou MKV"]];

function validRegex(v: string): string | null {
  try { new RegExp(v); return null; } catch { return "Expression régulière invalide."; }
}

export default function SousTitres() {
  const { s, patch } = useS();
  return (
    <Page>
      <SettingGroup title="Sous-titres">
        <SettingRow label="Mode" hint="« Obligatoires » fait échouer le téléchargement si aucune piste ne correspond.">
          <SelectField value={s.sub_mode} options={MODES} onChange={(v) => patch({ sub_mode: v })} />
        </SettingRow>
        <SettingRow label="Format de sortie">
          <SelectField value={s.sub_out} options={OUT} onChange={(v) => patch({ sub_out: v })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Choix des pistes">
        <SettingRow label="Langues" hint="Codes de langue séparés par des virgules, par ordre de préférence.">
          <ListField value={s.sub_langs} placeholder="fr, en" width={200} onChange={(v) => patch({ sub_langs: v })} />
        </SettingRow>
        <ToggleRow label="Télécharger plusieurs langues" hint="Sinon, seule la première langue disponible est conservée." checked={s.sub_multiple} onChange={(v) => patch({ sub_multiple: v })} />
        <ToggleRow label="Accepter les sous-titres automatiques" checked={s.sub_auto} onChange={(v) => patch({ sub_auto: v })} />
        <SettingRow label="Filtre sur le nom de la piste" hint="Expression régulière appliquée au libellé, par exemple ^(?!.*SDH).">
          <TextField value={s.sub_label_regex} mono width={200} validate={validRegex} onChange={(v) => patch({ sub_label_regex: v })} />
        </SettingRow>
      </SettingGroup>
    </Page>
  );
}
