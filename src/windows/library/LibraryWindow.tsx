import { useEffect, useState } from "react";
import { Titlebar } from "@/components/app/Titlebar";
import { cn } from "@/lib/utils";
import { on, subscribe } from "@/app/events";
import { HistoryView } from "./HistoryView";
import { SearchView } from "./SearchView";
import { SitesView } from "./SitesView";

type Section = "history" | "search" | "sites";
const SECTIONS: { id: Section; label: string }[] = [
  { id: "history", label: "Historique" },
  { id: "search", label: "Recherche" },
  { id: "sites", label: "Sites" },
];

function parse(v: unknown): Section | null {
  return v === "history" || v === "search" || v === "sites" ? v : null;
}

export default function LibraryWindow() {
  const [section, setSection] = useState<Section>(() => parse(new URLSearchParams(window.location.search).get("section")) ?? "history");

  useEffect(
    () => subscribe(on<string>("window://section", (v) => { const s = parse(v); if (s) setSection(s); })),
    [],
  );

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <Titlebar>
        <div role="tablist" aria-label="Sections" className="flex items-center gap-0.5 rounded-lg bg-muted p-0.5">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              role="tab"
              aria-selected={section === s.id}
              onClick={() => setSection(s.id)}
              className={cn(
                "h-7 rounded-md px-3 text-[13px] font-medium transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
                section === s.id ? "bg-background text-foreground shadow-xs" : "text-muted-foreground hover:text-foreground",
              )}
            >
              {s.label}
            </button>
          ))}
        </div>
        <div data-tauri-drag-region className="flex-1 self-stretch" />
      </Titlebar>
      <main className="min-h-0 flex-1">
        {section === "history" && <HistoryView />}
        {section === "search" && <SearchView />}
        {section === "sites" && <SitesView />}
      </main>
    </div>
  );
}
