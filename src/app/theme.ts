import { useEffect } from "react";
import { useSettings } from "@/stores/settings";

/** Applique le thème (sombre par défaut, clair ou système) et la densité. */
export function useTheme() {
  const prefs = useSettings((s) => s.prefs);
  useEffect(() => {
    const mode = prefs?.theme ?? "dark";
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = mode === "dark" || (mode === "system" && mq.matches);
      document.documentElement.classList.toggle("dark", dark);
    };
    apply();
    if (mode === "system") {
      mq.addEventListener("change", apply);
      return () => mq.removeEventListener("change", apply);
    }
  }, [prefs?.theme]);
  useEffect(() => {
    document.documentElement.dataset.density = prefs?.density ?? "comfortable";
  }, [prefs?.density]);
}
