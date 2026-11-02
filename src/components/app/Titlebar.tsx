import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** Barre de titre intégrée : zone de glisser-déposer de la fenêtre, avec réserve pour les boutons macOS. */
export function Titlebar({ children, className, leftPad = true }: { children?: ReactNode; className?: string; leftPad?: boolean }) {
  return (
    <div data-tauri-drag-region className={cn("flex h-12 shrink-0 items-center gap-2 pr-3 hairline", leftPad ? "pl-[84px]" : "pl-3", className)}>
      {children}
    </div>
  );
}
