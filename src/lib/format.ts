import type { Task, TaskState } from "@/types";

const nf1 = new Intl.NumberFormat("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const nf0 = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 0 });

export function bytes(n: number | null | undefined): string {
  if (n == null || !isFinite(n)) return "–";
  if (n < 1024) return `${nf0.format(n)} o`;
  const units = ["Ko", "Mo", "Go", "To"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v >= 100 ? nf0.format(v) : nf1.format(v)} ${units[i]}`;
}

export function speed(bps: number | null | undefined): string {
  if (!bps || bps <= 0) return "–";
  return `${bytes(bps)}/s`;
}

export function eta(secs: number | null | undefined): string {
  if (secs == null || !isFinite(secs)) return "–";
  const s = Math.round(secs);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${String(s % 60).padStart(2, "0")} s`;
  const h = Math.floor(m / 60);
  return `${h} h ${String(m % 60).padStart(2, "0")} min`;
}

export function duration(secs: number | null | undefined): string {
  if (secs == null || !isFinite(secs)) return "";
  const s = Math.round(secs);
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}` : `${m}:${String(r).padStart(2, "0")}`;
}

export function percent(p: number | null | undefined): string {
  return `${Math.max(0, Math.min(100, p ?? 0)).toFixed(0).padStart(2, " ")} %`;
}

export function dateTime(unix: number | null | undefined): string {
  if (!unix) return "";
  return new Date(unix * 1000).toLocaleString("fr-FR", { day: "2-digit", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
}

export function hostOf(url: string): string {
  try { return new URL(url).hostname.replace(/^www\./, ""); } catch { return url; }
}

export const STATE_LABEL: Record<TaskState, string> = {
  Queued: "En attente",
  Extracting: "Analyse",
  NeedsInput: "Question",
  Scheduled: "Planifié",
  Downloading: "Téléchargement",
  Recording: "Enregistrement",
  PostProcessing: "Traitement",
  Paused: "En pause",
  Finished: "Terminé",
  Failed: "Échec",
  Cancelled: "Annulé",
};

export const isActive = (s: TaskState) => s === "Extracting" || s === "Downloading" || s === "Recording" || s === "PostProcessing";
export const isDone = (s: TaskState) => s === "Finished" || s === "Failed" || s === "Cancelled";

export function taskTitle(t: Task): string {
  return t.title || t.options.title_hint || t.url;
}

/** Libellé d'état complet pour une ligne de file. */
export function stateText(t: Task): string {
  switch (t.state) {
    case "Scheduled":
      return t.scheduled_at ? `${t.error ? "Nouvelle tentative " : "Planifié "}${dateTime(t.scheduled_at)}` : "Planifié";
    case "Downloading":
    case "Recording":
    case "PostProcessing":
      return t.progress.stage ? t.progress.stage : STATE_LABEL[t.state];
    case "Failed":
      return t.error ?? "Échec";
    default:
      return STATE_LABEL[t.state];
  }
}

export const RESOLUTIONS: { label: string; value: number | null }[] = [
  { label: "Meilleure qualité (aucune limite)", value: null },
  { label: "8K (4320p)", value: 4320 },
  { label: "4K (2160p)", value: 2160 },
  { label: "1440p", value: 1440 },
  { label: "1080p", value: 1080 },
  { label: "720p", value: 720 },
  { label: "480p", value: 480 },
  { label: "360p", value: 360 },
  { label: "240p", value: 240 },
  { label: "144p", value: 144 },
];
