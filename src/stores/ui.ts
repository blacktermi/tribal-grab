import { create } from "zustand";

export type Confirm = "cancel_all" | "reset_queue" | null;

interface UiState {
  addOptionsOpen: boolean;
  addOptionsText: string;
  detailsUrl: string | null;
  scheduleId: number | null;
  confirm: Confirm;
  tgi: { json: string; info: import("@/types").IntegrationInfo } | null;
  focusTick: number;
  set: (p: Partial<Omit<UiState, "set" | "focusAdd">>) => void;
  focusAdd: () => void;
}

export const useUi = create<UiState>((set) => ({
  addOptionsOpen: false,
  addOptionsText: "",
  detailsUrl: null,
  scheduleId: null,
  confirm: null,
  tgi: null,
  focusTick: 0,
  set: (p) => set(p),
  focusAdd: () => set((s) => ({ focusTick: s.focusTick + 1 })),
}));
