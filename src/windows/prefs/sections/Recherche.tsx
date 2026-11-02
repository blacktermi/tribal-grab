import { SettingGroup } from "@/components/app/Setting";
import { Page, ToggleRow, useP } from "./controls";

const PREFIXES: [string, string][] = [
  ["ytsearch", "YouTube"], ["scsearch", "SoundCloud"], ["gvsearch", "Google Vidéos"],
  ["yvsearch", "Yahoo"], ["nicosearch", "Niconico"], ["bilisearch", "Bilibili"],
];

export default function Recherche() {
  const { p, patchPrefs } = useP();
  return (
    <Page>
      <SettingGroup title="Recherche">
        <ToggleRow
          label="Lancer une recherche depuis la barre d'ajout quand le texte n'est pas un lien"
          checked={p.search_in_main}
          onChange={(v) => patchPrefs({ search_in_main: v })}
        />
      </SettingGroup>

      <SettingGroup title="Fonctionnement">
        <div className="space-y-2 py-3 text-[12px] leading-relaxed text-muted-foreground">
          <p>
            Le texte saisi est transmis à yt-dlp avec le préfixe du moteur, par exemple{" "}
            <code className="num text-foreground">ytsearch10:mon texte</code> pour dix résultats YouTube.
            Les moteurs interrogés se choisissent dans la section « Moteurs de recherche ».
          </p>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1 pt-1">
            {PREFIXES.map(([k, v]) => (
              <div key={k} className="flex items-baseline gap-2">
                <code className="num w-[88px] shrink-0 text-foreground">{k}</code>
                <span>{v}</span>
              </div>
            ))}
          </div>
        </div>
      </SettingGroup>
    </Page>
  );
}
