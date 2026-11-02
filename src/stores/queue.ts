import { create } from "zustand";
import { ipc } from "@/lib/ipc";
import type { GrabEvent, Progress, SortKey, Task, TaskState } from "@/types";
import { isActive, isDone, taskTitle } from "@/lib/format";

export type Filter = "all" | "active" | "waiting" | "finished" | "failed";

interface QueueState {
  tasks: Record<number, Task>;
  loaded: boolean;
  selection: number[];
  anchor: number | null;
  sortKey: SortKey;
  sortDesc: boolean;
  filter: Filter;
  load: () => Promise<void>;
  apply: (e: GrabEvent) => void;
  select: (id: number, mode: "one" | "toggle" | "range") => void;
  selectAll: () => void;
  clearSelection: () => void;
  setSort: (k: SortKey, desc: boolean) => void;
  setFilter: (f: Filter) => void;
}

export const useQueue = create<QueueState>((set, get) => ({
  tasks: {},
  loaded: false,
  selection: [],
  anchor: null,
  sortKey: "Order",
  sortDesc: false,
  filter: "all",
  load: async () => {
    const list = await ipc.tasks("Order", false);
    const tasks: Record<number, Task> = {};
    for (const t of list) tasks[t.id] = t;
    set({ tasks, loaded: true });
  },
  apply: (e) => {
    if (typeof e === "string") return;
    if ("TaskAdded" in e) set((s) => ({ tasks: { ...s.tasks, [e.TaskAdded.id]: e.TaskAdded } }));
    else if ("TaskUpdated" in e) set((s) => ({ tasks: { ...s.tasks, [e.TaskUpdated.id]: e.TaskUpdated } }));
    else if ("TaskProgress" in e) {
      const { id, progress } = e.TaskProgress;
      set((s) => {
        const t = s.tasks[id];
        return t ? { tasks: { ...s.tasks, [id]: { ...t, progress: progress as Progress } } } : s;
      });
    } else if ("TaskRemoved" in e) {
      const id = e.TaskRemoved.id;
      set((s) => {
        const tasks = { ...s.tasks };
        delete tasks[id];
        return { tasks, selection: s.selection.filter((x) => x !== id) };
      });
    }
  },
  select: (id, mode) => {
    const ids = visibleIds(get());
    set((s) => {
      if (mode === "toggle") {
        return { selection: s.selection.includes(id) ? s.selection.filter((x) => x !== id) : [...s.selection, id], anchor: id };
      }
      if (mode === "range" && s.anchor != null) {
        const a = ids.indexOf(s.anchor), b = ids.indexOf(id);
        if (a >= 0 && b >= 0) return { selection: ids.slice(Math.min(a, b), Math.max(a, b) + 1) };
      }
      return { selection: [id], anchor: id };
    });
  },
  selectAll: () => set({ selection: visibleIds(get()) }),
  clearSelection: () => set({ selection: [], anchor: null }),
  setSort: (sortKey, sortDesc) => set({ sortKey, sortDesc }),
  setFilter: (filter) => set({ filter }),
}));

const stateGroup = (s: TaskState): Filter =>
  isActive(s) ? "active" : s === "Finished" ? "finished" : s === "Failed" || s === "Cancelled" ? "failed" : "waiting";

export function sortedTasks(tasks: Record<number, Task>, key: SortKey, desc: boolean, filter: Filter): Task[] {
  let list = Object.values(tasks);
  if (filter !== "all") list = list.filter((t) => stateGroup(t.state) === filter);
  const cmp: Record<SortKey, (a: Task, b: Task) => number> = {
    Order: (a, b) => a.position - b.position || a.id - b.id,
    Site: (a, b) => (a.site ?? "").localeCompare(b.site ?? "", "fr") || a.position - b.position,
    Playlist: (a, b) => (a.playlist ?? "").localeCompare(b.playlist ?? "", "fr") || a.position - b.position,
    Progress: (a, b) => a.progress.percent - b.progress.percent,
    Name: (a, b) => taskTitle(a).localeCompare(taskTitle(b), "fr", { sensitivity: "base" }),
  };
  list.sort(cmp[key]);
  return desc ? list.reverse() : list;
}

function visibleIds(s: QueueState): number[] {
  return sortedTasks(s.tasks, s.sortKey, s.sortDesc, s.filter).map((t) => t.id);
}

export function counts(tasks: Record<number, Task>) {
  const c = { all: 0, active: 0, waiting: 0, finished: 0, failed: 0, speed: 0 };
  for (const t of Object.values(tasks)) {
    c.all++;
    c[stateGroup(t.state)]++;
    if (isActive(t.state)) c.speed += t.progress.speed || 0;
  }
  return c;
}

export const canPause = (t: Task) => isActive(t.state) || t.state === "Queued" || t.state === "Scheduled";
export const canResume = (t: Task) => t.state === "Paused";
export const canRetry = (t: Task) => t.state === "Failed" || t.state === "Cancelled";
export const canCancel = (t: Task) => !isDone(t.state);
