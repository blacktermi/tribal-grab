import { SettingGroup, SettingRow } from "@/components/app/Setting";
import type { HistoryRetention } from "@/types";
import { Page, SelectField, useS } from "./controls";

const OPTIONS: readonly (readonly [HistoryRetention, string])[] = [
  ["Week", "1 semaine"], ["Month", "1 mois"], ["ThreeMonths", "3 mois"], ["SixMonths", "6 mois"], ["Year", "1 an"], ["Never", "Jamais"],
];

export default function Historique() {
  const { s, patch } = useS();
  return (
    <Page>
      <SettingGroup title="Historique">
        <SettingRow label="Conserver l'historique" hint="Les entrées plus anciennes sont supprimées automatiquement. Les fichiers téléchargés ne sont jamais touchés.">
          <SelectField value={s.history_retention} options={OPTIONS} width={160} onChange={(v) => patch({ history_retention: v })} />
        </SettingRow>
      </SettingGroup>
    </Page>
  );
}
