import { toast } from "sonner";
import { errMsg } from "@/lib/ipc";

/** Exécute une action et affiche l'erreur éventuelle dans un toast. */
export async function attempt<T>(p: Promise<T>, ok?: string | ((r: T) => string)): Promise<T | undefined> {
  try {
    const r = await p;
    if (ok) toast.success(typeof ok === "function" ? ok(r) : ok);
    return r;
  } catch (e) {
    toast.error(errMsg(e));
    return undefined;
  }
}
