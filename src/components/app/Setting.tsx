import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/** Ligne de réglage : libellé et aide à gauche, contrôle à droite. */
export function SettingRow({ label, hint, children, className }: { label: string; hint?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex items-start justify-between gap-6 border-b border-border/60 py-3 last:border-b-0", className)}>
      <div className="min-w-0 max-w-[58%]">
        <div className="text-[13px] font-medium">{label}</div>
        {hint && <div className="mt-0.5 text-[12px] leading-snug text-muted-foreground">{hint}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

export function SettingGroup({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="mb-6">
      {title && <h3 className="mb-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">{title}</h3>}
      <div className="rounded-lg border border-border bg-card px-4">{children}</div>
    </section>
  );
}
