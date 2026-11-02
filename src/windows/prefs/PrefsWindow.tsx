import { lazy, Suspense, useEffect, useState, type ComponentType } from "react";
import {
  AppWindow, Cable, Clock, Download, FolderOpen, Gauge, Globe, Languages, Palette, Puzzle, Search, SlidersHorizontal,
  Sparkles, Volume2, Wrench, type LucideIcon,
} from "lucide-react";
import { Titlebar } from "@/components/app/Titlebar";
import { on, subscribe } from "@/app/events";
import { cn } from "@/lib/utils";
import { useSettings } from "@/stores/settings";

interface Section { id: string; label: string; icon: LucideIcon; C: ComponentType }

const lz = (f: () => Promise<{ default: ComponentType }>) => lazy(f);

const SECTIONS: Section[] = [
  { id: "general", label: "Général", icon: SlidersHorizontal, C: lz(() => import("./sections/General")) },
  { id: "fichiers", label: "Fichiers", icon: FolderOpen, C: lz(() => import("./sections/Fichiers")) },
  { id: "qualite", label: "Qualité", icon: Gauge, C: lz(() => import("./sections/Qualite")) },
  { id: "post", label: "Après le téléchargement", icon: Sparkles, C: lz(() => import("./sections/Post")) },
  { id: "soustitres", label: "Sous-titres", icon: Languages, C: lz(() => import("./sections/SousTitres")) },
  { id: "reseau", label: "Réseau et comptes", icon: Globe, C: lz(() => import("./sections/Reseau")) },
  { id: "sons", label: "Sons et notifications", icon: Volume2, C: lz(() => import("./sections/Sons")) },
  { id: "historique", label: "Historique", icon: Clock, C: lz(() => import("./sections/Historique")) },
  { id: "integrations", label: "Intégrations", icon: Puzzle, C: lz(() => import("./sections/Integrations")) },
  { id: "extensions", label: "Extensions", icon: Cable, C: lz(() => import("./sections/Extensions")) },
  { id: "recherche", label: "Recherche", icon: Search, C: lz(() => import("./sections/Recherche")) },
  { id: "moteurs", label: "Moteurs de recherche", icon: Download, C: lz(() => import("./sections/Moteurs")) },
  { id: "avance", label: "Avancé", icon: Wrench, C: lz(() => import("./sections/Avance")) },
  { id: "apparence", label: "Apparence", icon: Palette, C: lz(() => import("./sections/Apparence")) },
];

const ids = SECTIONS.map((s) => s.id);
const valid = (id: string | null | undefined): id is string => !!id && ids.includes(id);

function initialSection(): string {
  const q = new URLSearchParams(window.location.search).get("section");
  return valid(q) ? q : "general";
}

export default function PrefsWindow() {
  const [current, setCurrent] = useState(initialSection);
  const ready = useSettings((s) => s.settings !== null && s.prefs !== null);

  useEffect(() => subscribe(on<string>("window://section", (id) => { if (valid(id)) setCurrent(id); })), []);

  const section = SECTIONS.find((s) => s.id === current) ?? SECTIONS[0];
  const Body = section.C;

  return (
    <div className="flex h-full flex-col bg-background">
      <Titlebar>
        <h1 className="pointer-events-none text-[13px] font-semibold">{section.label}</h1>
      </Titlebar>
      <div className="flex min-h-0 flex-1">
        <nav className="w-[210px] shrink-0 overflow-y-auto border-r border-border bg-surface p-2" aria-label="Sections des préférences">
          {SECTIONS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              onClick={() => setCurrent(id)}
              aria-current={id === current ? "page" : undefined}
              className={cn(
                "flex h-[30px] w-full items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] text-muted-foreground transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
                id === current ? "bg-row-selected font-medium text-foreground" : "hover:bg-row-hover hover:text-foreground",
              )}
            >
              <Icon className={cn("size-4 shrink-0", id === current && "text-primary")} />
              <span className="truncate">{label}</span>
            </button>
          ))}
        </nav>
        <main className="min-w-0 flex-1 overflow-y-auto">
          {ready ? (
            <Suspense fallback={null}>
              <Body key={section.id} />
            </Suspense>
          ) : (
            <div className="flex h-full items-center justify-center text-[12px] text-muted-foreground">
              <AppWindow className="mr-2 size-4" />Chargement des préférences…
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
