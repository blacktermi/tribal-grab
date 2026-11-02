import { useEffect, useState } from "react";
import { toast } from "sonner";

/** Valeur qui ne suit `value` qu'après `ms` millisecondes de calme. */
export function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

export async function copyText(text: string, ok = "Lien copié"): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(ok);
  } catch {
    toast.error("Impossible de copier dans le presse-papiers");
  }
}

export function readStore(key: string): string | null {
  try { return localStorage.getItem(key); } catch { return null; }
}
export function writeStore(key: string, value: string): void {
  try { localStorage.setItem(key, value); } catch { /* stockage indisponible */ }
}

export const plural = (n: number, one: string, many: string) => `${n} ${n > 1 ? many : one}`;
