import { StrictMode, lazy, Suspense, useEffect } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@/styles/index.css";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useSettings } from "@/stores/settings";
import { useTheme } from "@/app/theme";

const MainWindow = lazy(() => import("@/windows/main/MainWindow"));
const PrefsWindow = lazy(() => import("@/windows/prefs/PrefsWindow"));
const LibraryWindow = lazy(() => import("@/windows/library/LibraryWindow"));
const BrowserWindow = lazy(() => import("@/windows/browser/BrowserWindow"));
const TrayWindow = lazy(() => import("@/windows/tray/TrayWindow"));

type Internals = { metadata?: { currentWebview?: { label?: string } } };
const label = (window as unknown as { __TAURI_INTERNALS__?: Internals }).__TAURI_INTERNALS__?.metadata?.currentWebview?.label ?? "main";

function Root() {
  const load = useSettings((s) => s.load);
  useTheme();
  useEffect(() => {
    load();
    // Pas de menu contextuel du navigateur, sauf dans les champs de texte.
    const block = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;
      if (!t?.closest("input, textarea, .selectable")) e.preventDefault();
    };
    document.addEventListener("contextmenu", block);
    return () => document.removeEventListener("contextmenu", block);
  }, [load]);
  const Win = label === "prefs" ? PrefsWindow : label === "library" ? LibraryWindow : label === "browser-ui" ? BrowserWindow : label === "tray" ? TrayWindow : MainWindow;
  return (
    <TooltipProvider delayDuration={400}>
      <Suspense fallback={null}>
        <Win />
      </Suspense>
      <Toaster position="bottom-center" closeButton={false} />
    </TooltipProvider>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
