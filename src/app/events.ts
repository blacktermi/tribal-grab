import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { GrabEvent } from "@/types";

export function onGrabEvent(cb: (e: GrabEvent) => void): Promise<UnlistenFn> {
  return listen<GrabEvent>("grab://event", (e) => cb(e.payload));
}

export function on<T = unknown>(name: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(name, (e) => cb(e.payload));
}

/** À utiliser dans un `useEffect` : retourne la fonction de nettoyage. */
export function subscribe(...fns: Promise<UnlistenFn>[]): () => void {
  return () => fns.forEach((f) => f.then((u) => u()).catch(() => {}));
}
