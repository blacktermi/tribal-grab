import { useCallback, useMemo, useRef } from "react";
import { useQueue, sortedTasks } from "@/stores/queue";
import { TaskRow } from "./TaskRow";
import { TaskContextMenu } from "./TaskContextMenu";
import { useSettings } from "@/stores/settings";
import { ipc } from "@/lib/ipc";
import { attempt } from "@/components/app/errors";
import { EmptyState } from "./EmptyState";

export function QueueList({ onDetails, onSchedule }: { onDetails: (url: string) => void; onSchedule: (id: number) => void }) {
  const tasks = useQueue((s) => s.tasks);
  const sortKey = useQueue((s) => s.sortKey);
  const sortDesc = useQueue((s) => s.sortDesc);
  const filter = useQueue((s) => s.filter);
  const selection = useQueue((s) => s.selection);
  const select = useQueue((s) => s.select);
  const clearSelection = useQueue((s) => s.clearSelection);
  const prefs = useSettings((s) => s.prefs);
  const dragId = useRef<number | null>(null);

  const list = useMemo(() => sortedTasks(tasks, sortKey, sortDesc, filter), [tasks, sortKey, sortDesc, filter]);
  const selected = useMemo(() => new Set(selection), [selection]);
  const canReorder = sortKey === "Order" && !sortDesc && filter === "all";

  const onClickRow = useCallback((id: number, e: React.MouseEvent) => select(id, e.shiftKey ? "range" : e.metaKey || e.ctrlKey ? "toggle" : "one"), [select]);

  if (list.length === 0) return <EmptyState filtered={Object.keys(tasks).length > 0} />;

  return (
    <div className="min-h-0 flex-1 overflow-y-auto" onClick={(e) => e.target === e.currentTarget && clearSelection()}>
      {list.map((t, i) => (
        <TaskContextMenu key={t.id} task={t} selectedIds={selection} onDetails={onDetails} onSchedule={onSchedule}>
          <div>
            <TaskRow
              task={t}
              selected={selected.has(t.id)}
              showThumb={prefs?.show_thumbnails ?? true}
              compact={prefs?.density === "compact"}
              onSelect={(e) => onClickRow(t.id, e)}
              onDragStart={canReorder ? (e) => { dragId.current = t.id; e.dataTransfer.effectAllowed = "move"; e.dataTransfer.setData("application/x-tribal-task", String(t.id)); } : undefined}
              onDropOn={canReorder ? (e) => {
                if (dragId.current == null) return;
                e.preventDefault();
                e.stopPropagation();
                const id = dragId.current;
                dragId.current = null;
                if (id !== t.id) attempt(ipc.moveTask(id, i));
              } : undefined}
            />
          </div>
        </TaskContextMenu>
      ))}
      <div className="h-6" onClick={clearSelection} />
    </div>
  );
}
