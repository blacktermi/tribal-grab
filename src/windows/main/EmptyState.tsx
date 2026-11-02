import { Clipboard, MousePointerClick, TerminalSquare } from "lucide-react";
import { useQueue } from "@/stores/queue";

/** État vide : dit quoi faire (coller un lien), sans habillage promotionnel. */
export function EmptyState({ filtered }: { filtered: boolean }) {
  const setFilter = useQueue((s) => s.setFilter);
  if (filtered) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 text-[13px] text-muted-foreground">
        <span>Aucun téléchargement dans cette catégorie.</span>
        <button className="text-primary hover:underline" onClick={() => setFilter("all")}>Tout afficher</button>
      </div>
    );
  }
  return (
    <div className="flex flex-1 items-center justify-center px-8">
      <div className="w-full max-w-[420px]">
        <h2 className="text-[15px] font-semibold">Collez un lien pour commencer</h2>
        <p className="mt-1 text-[13px] leading-relaxed text-muted-foreground">
          Tribal Grab reconnaît les vidéos et les pistes audio de la plupart des sites. Le téléchargement démarre dès que le lien est ajouté.
        </p>
        <ul className="mt-5 space-y-2.5 text-[12.5px] text-muted-foreground">
          <li className="flex items-center gap-2.5"><Clipboard className="size-4 shrink-0" strokeWidth={1.6} /><span><kbd className="num rounded border border-border bg-muted px-1.5 py-0.5 text-[11px] text-foreground">⌘V</kbd> n'importe où dans la fenêtre colle et ajoute les liens du presse-papiers.</span></li>
          <li className="flex items-center gap-2.5"><MousePointerClick className="size-4 shrink-0" strokeWidth={1.6} /><span>Glissez un lien, un texte ou un fichier de liens sur la fenêtre.</span></li>
          <li className="flex items-center gap-2.5"><TerminalSquare className="size-4 shrink-0" strokeWidth={1.6} /><span>Depuis un terminal : <code className="num text-foreground">tribal-grab get &lt;lien&gt;</code></span></li>
        </ul>
      </div>
    </div>
  );
}
