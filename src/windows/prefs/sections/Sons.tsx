import { useEffect, useState } from "react";
import { Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SettingGroup, SettingRow } from "@/components/app/Setting";
import { attempt } from "@/components/app/errors";
import { ipc } from "@/lib/ipc";
import type { AppPrefs } from "@/types";
import { Page, SelectField, ToggleRow, useP, useS } from "./controls";

type SoundKey = "sound_added" | "sound_finished" | "sound_failed" | "sound_queue_finished";
const SOUNDS: [SoundKey, string][] = [
  ["sound_added", "Lien ajouté"],
  ["sound_finished", "Téléchargement terminé"],
  ["sound_failed", "Téléchargement échoué"],
  ["sound_queue_finished", "File terminée"],
];
const NONE = "__none__";

export default function Sons() {
  const { s, patch } = useS();
  const { p, patchPrefs } = useP();
  const [list, setList] = useState<string[]>([]);
  useEffect(() => {
    void attempt(ipc.listSounds()).then((l) => { if (l) setList(l); });
  }, []);

  const options = (cur: string): readonly (readonly [string, string])[] => {
    const names = cur && cur !== "" && !list.includes(cur) ? [...list, cur] : list;
    return [[NONE, "Aucun"], ...names.map((n) => [n, n] as const)];
  };

  return (
    <Page>
      <SettingGroup title="Notifications">
        <ToggleRow label="Notifications du système" hint="Affichées à la fin d'un téléchargement ou en cas d'échec." checked={s.notifications} onChange={(v) => patch({ notifications: v })} />
        <ToggleRow label="Afficher la fenêtre quand une question est posée" checked={p.focus_on_question} onChange={(v) => patchPrefs({ focus_on_question: v })} />
      </SettingGroup>

      <SettingGroup title="Sons">
        <ToggleRow label="Activer les sons" checked={p.sounds} onChange={(v) => patchPrefs({ sounds: v })} />
        {SOUNDS.map(([key, label]) => {
          const cur = p[key as keyof AppPrefs] as string;
          return (
            <SettingRow key={key} label={label}>
              <SelectField
                width={170}
                disabled={!p.sounds}
                value={cur === "" ? NONE : cur}
                options={options(cur)}
                onChange={(v) => patchPrefs({ [key]: v === NONE ? "" : v } as Partial<AppPrefs>)}
              />
              <Button variant="outline" size="icon-sm" className="size-[30px]" title="Écouter" aria-label={`Écouter : ${label}`}
                disabled={cur === ""} onClick={() => attempt(ipc.playSound(cur))}>
                <Play className="size-3.5" />
              </Button>
            </SettingRow>
          );
        })}
      </SettingGroup>
    </Page>
  );
}
