import { useMemo } from "react";
import { useQueue } from "@/stores/queue";
import { QualityDialog } from "./QualityDialog";
import { SubtitlesDialog } from "./SubtitlesDialog";
import { LinkFilterDialog } from "./LinkFilterDialog";
import { DuplicateDialog } from "./DuplicateDialog";
import { DestinationDialog } from "./DestinationDialog";

/** Affiche la question de la première tâche qui attend une réponse (NeedsInput). */
export function QuestionHost() {
  const tasks = useQueue((s) => s.tasks);
  const task = useMemo(() => Object.values(tasks).filter((t) => t.state === "NeedsInput" && t.question).sort((a, b) => a.id - b.id)[0], [tasks]);
  const q = task?.question;
  if (!task || !q) return null;
  const key = `${task.id}-${Object.keys(q)[0]}`;
  if ("Quality" in q) return <QualityDialog key={key} task={task} choices={q.Quality.choices} />;
  if ("Subtitles" in q) return <SubtitlesDialog key={key} task={task} tracks={q.Subtitles.tracks} />;
  if ("Links" in q) return <LinkFilterDialog key={key} task={task} title={q.Links.title} links={q.Links.links} />;
  if ("Duplicate" in q) return <DuplicateDialog key={key} task={task} previous={q.Duplicate.previous} />;
  if ("Destination" in q) return <DestinationDialog key={key} task={task} suggested={q.Destination.suggested} />;
  return null;
}
