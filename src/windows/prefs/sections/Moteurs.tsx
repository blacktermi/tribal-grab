import { SettingGroup } from "@/components/app/Setting";
import { CheckRow, Note, Page, useS } from "./controls";

const ENGINES: [string, string][] = [
  ["youtube", "YouTube"], ["soundcloud", "SoundCloud"], ["google", "Google"],
  ["yahoo", "Yahoo"], ["niconico", "Niconico"], ["bilibili", "Bilibili"],
];

export default function Moteurs() {
  const { s, patch } = useS();
  const toggle = (id: string, on: boolean) => {
    const rest = s.search_engines.filter((x) => x !== id);
    patch({ search_engines: on ? [...rest, id] : rest });
  };
  return (
    <Page>
      <SettingGroup title="Moteurs de recherche">
        {ENGINES.map(([id, label]) => (
          <CheckRow key={id} label={label} checked={s.search_engines.includes(id)} onChange={(v) => toggle(id, v)} />
        ))}
      </SettingGroup>
      {s.search_engines.length === 0 && <Note warn>Aucun moteur coché : la recherche ne renverra aucun résultat.</Note>}
    </Page>
  );
}
