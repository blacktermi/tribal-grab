import { SettingGroup, SettingRow } from "@/components/app/Setting";
import type { AppPrefs } from "@/types";
import { Page, SelectField, ToggleRow, useP } from "./controls";

const THEMES: readonly (readonly [AppPrefs["theme"], string])[] = [["dark", "Sombre"], ["light", "Clair"], ["system", "Système"]];
const DENSITY: readonly (readonly [AppPrefs["density"], string])[] = [["comfortable", "Confortable"], ["compact", "Compact"]];

export default function Apparence() {
  const { p, patchPrefs } = useP();
  return (
    <Page>
      <SettingGroup title="Affichage">
        <SettingRow label="Thème">
          <SelectField value={p.theme} options={THEMES} width={150} onChange={(v) => patchPrefs({ theme: v })} />
        </SettingRow>
        <SettingRow label="Densité des listes">
          <SelectField value={p.density} options={DENSITY} width={150} onChange={(v) => patchPrefs({ density: v })} />
        </SettingRow>
        <ToggleRow label="Afficher les miniatures" checked={p.show_thumbnails} onChange={(v) => patchPrefs({ show_thumbnails: v })} />
      </SettingGroup>

      <SettingGroup title="Dock et barre de menus">
        <ToggleRow
          label="Masquer l'icône du Dock"
          hint="L'icône de la barre de menus reste alors active."
          checked={p.hide_dock}
          onChange={(v) => patchPrefs(v ? { hide_dock: true, menubar_icon: true } : { hide_dock: false })}
        />
        <ToggleRow
          label="Afficher l'icône dans la barre de menus"
          checked={p.hide_dock || p.menubar_icon}
          disabled={p.hide_dock}
          onChange={(v) => patchPrefs({ menubar_icon: v })}
        />
        <ToggleRow label="Afficher le débit dans la barre de menus" checked={p.menubar_speed} onChange={(v) => patchPrefs({ menubar_speed: v })} />
      </SettingGroup>
    </Page>
  );
}
