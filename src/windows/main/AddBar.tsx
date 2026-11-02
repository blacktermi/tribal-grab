import { useEffect, useRef, useState } from "react";
import { ChevronDown, Plus, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ipc, errMsg } from "@/lib/ipc";
import { toast } from "sonner";
import { useUi } from "@/stores/ui";
import { useSettings } from "@/stores/settings";
import { emit } from "@tauri-apps/api/event";

const HAS_LINK = /https?:\/\/\S+/i;

export async function addFromText(text: string, opts: { quiet?: boolean } = {}): Promise<boolean> {
  const t = text.trim();
  if (!t) return false;
  try {
    let src = t;
    if (!HAS_LINK.test(src) && !/\s/.test(src) && src.includes(".")) src = `https://${src}`;
    const ids = await ipc.addText(src);
    if (!opts.quiet) toast.success(ids.length === 1 ? "Lien ajouté à la file" : `${ids.length} liens ajoutés à la file`);
    return true;
  } catch (e) {
    toast.error(errMsg(e));
    return false;
  }
}

/** Barre d'ajout : un lien, plusieurs liens collés, ou (si activé) une recherche. */
export function AddBar() {
  const [value, setValue] = useState("");
  const ref = useRef<HTMLInputElement>(null);
  const tick = useUi((s) => s.focusTick);
  const setUi = useUi((s) => s.set);
  const searchInMain = useSettings((s) => s.prefs?.search_in_main);

  useEffect(() => { if (tick > 0) ref.current?.focus(); }, [tick]);

  async function submit() {
    const v = value.trim();
    if (!v) return;
    const looksLikeLink = HAS_LINK.test(v) || (!/\s/.test(v) && v.includes("."));
    if (!looksLikeLink && searchInMain) {
      await ipc.openWindow("library", "search");
      setTimeout(() => emit("library://query", v), 700);
      setValue("");
      return;
    }
    if (await addFromText(v)) setValue("");
  }

  return (
    <div className="no-drag flex min-w-0 flex-1 items-center gap-2">
      <div className="relative flex min-w-0 flex-1 items-center">
        {searchInMain ? <Search className="pointer-events-none absolute left-2.5 size-3.5 text-muted-foreground" /> : <Plus className="pointer-events-none absolute left-2.5 size-3.5 text-muted-foreground" />}
        <input
          ref={ref}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          onPaste={(e) => {
            const text = e.clipboardData.getData("text");
            if (text && (text.match(/https?:\/\//gi)?.length ?? 0) > 1) {
              e.preventDefault();
              e.stopPropagation();
              addFromText(text).then((ok) => ok && setValue(""));
            }
          }}
          placeholder={searchInMain ? "Coller un lien, ou chercher une vidéo" : "Coller un lien ici"}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          className="h-8 w-full rounded-md border border-input bg-card pl-8 pr-3 text-[13px] outline-none transition-colors placeholder:text-muted-foreground focus:border-ring focus:ring-2 focus:ring-ring/30"
        />
      </div>
      <div className="flex">
        <Button size="sm" className="h-8 rounded-r-none px-3" onClick={submit} disabled={!value.trim()}>Ajouter</Button>
        <Button size="sm" className="h-8 rounded-l-none border-l border-primary-foreground/20 px-1.5" title="Ajouter avec options…" onClick={() => setUi({ addOptionsOpen: true, addOptionsText: value })}>
          <ChevronDown className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}
