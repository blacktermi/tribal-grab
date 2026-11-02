import { useState } from "react";
import { File, Film, Music, Image as ImageIcon } from "lucide-react";
import { cn } from "@/lib/utils";

/** Miniature distante, affichée comme image seulement (jamais comme HTML). */
export function Thumb({ src, className, kind = "video" }: { src?: string | null; className?: string; kind?: "video" | "audio" | "image" | "file" }) {
  const [bad, setBad] = useState(false);
  const Icon = kind === "file" ? File : kind === "audio" ? Music : kind === "image" ? ImageIcon : Film;
  return (
    <div className={cn("relative flex shrink-0 items-center justify-center overflow-hidden rounded-md bg-muted text-muted-foreground/60", className)}>
      {src && !bad ? (
        <img src={src} alt="" loading="lazy" referrerPolicy="no-referrer" draggable={false} onError={() => setBad(true)} className="size-full object-cover" />
      ) : (
        <Icon className="size-4" strokeWidth={1.5} />
      )}
    </div>
  );
}
