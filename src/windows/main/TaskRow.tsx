import { memo } from "react";
import { Folder, Pause, Play, RotateCcw, X, ArrowUp, Check, Clock } from "lucide-react";
import { cn } from "@/lib/utils";
import { Thumb } from "@/components/app/Thumb";
import { bytes, eta, hostOf, isActive, percent, speed, stateText, taskTitle } from "@/lib/format";
import { canPause, canResume, canRetry } from "@/stores/queue";
import type { Task } from "@/types";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";

function Bar({ task }: { task: Task }) {
  const { state, progress } = task;
  const indeterminate = state === "Extracting" || state === "PostProcessing" || (isActive(state) && !progress.total && progress.percent <= 0);
  const value = state === "Finished" ? 100 : progress.percent;
  const color = state === "Failed" ? "bg-destructive" : state === "Paused" || state === "Queued" || state === "Scheduled" ? "bg-muted-foreground/50" : "bg-primary";
  if (state === "Cancelled") return <div className="h-[3px]" />;
  return (
    <div className="relative h-[3px] w-full overflow-hidden rounded-full bg-muted">
      {indeterminate ? (
        <div className="absolute inset-y-0 w-1/4 animate-[indet_1.3s_ease-in-out_infinite] rounded-full bg-primary/80" />
      ) : (
        <div className={cn("h-full rounded-full transition-[width] duration-500 ease-out", color)} style={{ width: `${Math.max(0, Math.min(100, value))}%` }} />
      )}
    </div>
  );
}

function Actions({ task }: { task: Task }) {
  const btn = "flex size-6 items-center justify-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground";
  const stop = (e: React.MouseEvent) => e.stopPropagation();
  return (
    <div className="flex w-[78px] shrink-0 items-center justify-end gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-data-[selected=true]:opacity-100" onClick={stop} onDoubleClick={stop}>
      {canPause(task) && <button className={btn} title="Mettre en pause" onClick={() => attempt(ipc.pause(task.id))}><Pause className="size-3.5" /></button>}
      {canResume(task) && <button className={btn} title="Reprendre" onClick={() => attempt(ipc.resume(task.id))}><Play className="size-3.5" /></button>}
      {canRetry(task) && <button className={btn} title="Relancer" onClick={() => attempt(ipc.retry(task.id))}><RotateCcw className="size-3.5" /></button>}
      {task.state === "Finished" && task.output_path && (
        <button className={btn} title="Afficher dans le Finder" onClick={() => attempt(ipc.revealInFinder(task.output_path!))}><Folder className="size-3.5" /></button>
      )}
      <button className={btn} title="Retirer de la file" onClick={() => attempt(ipc.remove(task.id))}><X className="size-3.5" /></button>
    </div>
  );
}

export const TaskRow = memo(function TaskRow({ task, selected, showThumb, compact, onSelect, onDragStart, onDropOn }: {
  task: Task;
  selected: boolean;
  showThumb: boolean;
  compact: boolean;
  onSelect: (e: React.MouseEvent) => void;
  onDragStart?: (e: React.DragEvent) => void;
  onDropOn?: (e: React.DragEvent) => void;
}) {
  const active = isActive(task.state);
  const showNumbers = task.state === "Downloading" || task.state === "Recording";
  const failed = task.state === "Failed";
  const site = task.site || hostOf(task.url);
  const total = task.progress.total;
  const received = task.progress.downloaded;
  const showSize = showNumbers || task.state === "Paused" || task.state === "Queued" || task.state === "Scheduled";
  const sizeText = !showSize ? null : total ? `${bytes(received)} / ${bytes(total)}` : received > 0 ? `${bytes(received)} reçus` : null;
  return (
    <div
      data-selected={selected}
      draggable={!!onDragStart}
      onDragStart={onDragStart}
      onDragOver={(e) => onDropOn && e.preventDefault()}
      onDrop={onDropOn}
      onClick={onSelect}
      onDoubleClick={() => task.state === "Finished" && task.output_path && attempt(ipc.openPath(task.output_path))}
      className={cn(
        "group flex items-center gap-3 border-b border-border/50 px-4 transition-colors",
        compact ? "py-1.5" : "py-2.5",
        selected ? "bg-row-selected" : "hover:bg-row-hover",
      )}
    >
      {showThumb && <Thumb src={task.thumbnail} className={cn("rounded-[5px]", compact ? "h-8 w-14" : "h-[38px] w-[68px]")} kind={task.is_file ? "file" : task.options.post_mode === "AudioOnly" || task.options.audio_format ? "audio" : "video"} />}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          {task.priority === "High" && <ArrowUp className="size-3 shrink-0 text-primary" strokeWidth={2.5} aria-label="Priorité haute" />}
          <span className={cn("truncate text-[13px] font-medium", task.state === "Cancelled" && "text-muted-foreground line-through")}>{taskTitle(task)}</span>
        </div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
          <span className="shrink-0">{site}</span>
          {task.playlist && <><span>·</span><span className="truncate">{task.playlist}</span></>}
          <span>·</span>
          <span className={cn("truncate", failed && "text-destructive")} title={failed ? (task.error ?? "") : undefined}>
            {task.state === "Finished" ? <span className="inline-flex items-center gap-1"><Check className="size-3" />Terminé{total ? ` · ${bytes(total)}` : ""}</span> : task.state === "Scheduled" ? <span className="inline-flex items-center gap-1"><Clock className="size-3" />{stateText(task)}</span> : stateText(task)}
          </span>
          {sizeText && <><span>·</span><span className="num shrink-0">{sizeText}</span></>}
        </div>
        {(active || task.state === "Paused" || task.state === "Queued" || task.state === "Scheduled" || failed) && <div className="mt-1.5"><Bar task={task} /></div>}
      </div>
      <div className="num hidden w-[250px] shrink-0 items-center justify-end gap-3 text-right text-[12px] text-muted-foreground sm:flex">
        {showNumbers ? (
          <>
            <span className="w-[44px] text-foreground">{percent(task.progress.percent)}</span>
            <span className="w-[86px]">{speed(task.progress.speed)}</span>
            <span className="w-[84px]">{eta(task.progress.eta_secs)}</span>
          </>
        ) : task.state === "Paused" && total ? (
          <span className="w-[44px]">{percent(task.progress.percent)}</span>
        ) : null}
      </div>
      <Actions task={task} />
    </div>
  );
});
