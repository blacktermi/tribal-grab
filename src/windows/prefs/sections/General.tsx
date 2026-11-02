import { SettingGroup, SettingRow } from "@/components/app/Setting";
import type { AfterQueue, DuplicatePolicy, UnsupportedLink } from "@/types";
import { Note, NumField, Page, SelectField, ToggleRow, useS } from "./controls";

const AFTER: readonly (readonly [AfterQueue, string])[] = [
  ["Nothing", "Rien"], ["Quit", "Quitter l'application"], ["Sleep", "Mettre le Mac en veille"],
];
const UNSUPPORTED: readonly (readonly [UnsupportedLink, string])[] = [
  ["Ignore", "Ignorer"], ["Guided", "Ouvrir l'extraction guidée"],
];
const DUPLICATE: readonly (readonly [DuplicatePolicy, string])[] = [
  ["Ask", "Demander"], ["Redownload", "Retélécharger"], ["Skip", "Ignorer"],
];

export default function General() {
  const { s, patch } = useS();
  return (
    <Page>
      <SettingGroup title="File d'attente">
        <ToggleRow label="Démarrer automatiquement" hint="Lance chaque téléchargement dès qu'il est ajouté." checked={s.auto_start} onChange={(v) => patch({ auto_start: v })} />
        <SettingRow label="Téléchargements simultanés" hint="Nombre de fichiers téléchargés en même temps.">
          <NumField value={s.max_concurrent} min={1} max={16} onChange={(n) => patch({ max_concurrent: n })} />
        </SettingRow>
        <ToggleRow label="Mode automatisé" hint="Ne pose aucune question : les choix par défaut sont appliqués." checked={s.automated_mode} onChange={(v) => patch({ automated_mode: v })} />
        <SettingRow label="Une fois la file terminée">
          <SelectField value={s.after_queue} options={AFTER} onChange={(v) => patch({ after_queue: v })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Débit">
        <SettingRow
          label="Connexions par fichier"
          hint={s.connections > 8
            ? <Note warn>Au-delà de 8 connexions, certains sites bloquent ou ralentissent les téléchargements.</Note>
            : "Les fichiers sont découpés en plusieurs parties téléchargées en parallèle."}
        >
          <NumField value={s.connections} min={1} max={32} onChange={(n) => patch({ connections: n })} />
        </SettingRow>
        <SettingRow label="Limite de débit" hint="0 pour ne pas limiter.">
          <NumField value={s.speed_limit_kbps} min={0} max={10_000_000} suffix="Ko/s" width={96} onChange={(n) => patch({ speed_limit_kbps: n })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Échecs et doublons">
        <SettingRow label="Tentatives en cas d'échec" hint="Nombre de nouvelles tentatives automatiques.">
          <NumField value={s.retry_max} min={0} max={20} onChange={(n) => patch({ retry_max: n })} />
        </SettingRow>
        <SettingRow label="Délai avant la première relance" hint="Le délai double à chaque nouvelle tentative.">
          <NumField value={s.retry_base_secs} min={1} max={3600} suffix="s" onChange={(n) => patch({ retry_base_secs: n })} />
        </SettingRow>
        <SettingRow label="Lien non pris en charge">
          <SelectField value={s.unsupported_link} options={UNSUPPORTED} onChange={(v) => patch({ unsupported_link: v })} />
        </SettingRow>
        <SettingRow label="Vidéo déjà téléchargée">
          <SelectField value={s.duplicate_policy} options={DUPLICATE} onChange={(v) => patch({ duplicate_policy: v })} />
        </SettingRow>
      </SettingGroup>
    </Page>
  );
}
