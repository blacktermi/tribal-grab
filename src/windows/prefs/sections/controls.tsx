import { useEffect, useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Checkbox } from "@/components/ui/checkbox";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SettingRow } from "@/components/app/Setting";
import { cn } from "@/lib/utils";
import { useSettings } from "@/stores/settings";
import type { AppPrefs, Settings } from "@/types";

/** Réglages du moteur : la fenêtre ne monte les sections qu'une fois les réglages chargés. */
export function useS() {
  const s = useSettings((st) => st.settings) as Settings;
  const patch = useSettings((st) => st.patch);
  return { s, patch };
}
export function useP() {
  const p = useSettings((st) => st.prefs) as AppPrefs;
  const patchPrefs = useSettings((st) => st.patchPrefs);
  return { p, patchPrefs };
}

export const fieldCls = "h-[30px] text-[13px] md:text-[13px] px-2.5";
export const btnCls = "h-[30px] px-3 text-[13px]";

export function Page({ children }: { children: ReactNode }) {
  return <div className="mx-auto w-full max-w-[640px] px-6 py-5">{children}</div>;
}

export function Note({ children, warn, className }: { children: ReactNode; warn?: boolean; className?: string }) {
  return (
    <div className={cn("flex items-start gap-1.5 text-[12px] leading-snug", warn ? "text-primary" : "text-muted-foreground", className)}>
      {warn && <AlertTriangle className="mt-px size-3.5 shrink-0" />}
      <div>{children}</div>
    </div>
  );
}

/** Champ numérique compact : accepte la saisie libre, borne à la perte du focus. */
export function NumField({
  value, onChange, min, max, suffix, width = 76, disabled,
}: { value: number; onChange: (n: number) => void; min: number; max: number; suffix?: string; width?: number; disabled?: boolean }) {
  const [text, setText] = useState(String(value));
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(String(value));
  }, [value]);
  const commit = (raw: string) => {
    const n = Number(raw);
    if (raw.trim() !== "" && Number.isFinite(n)) onChange(Math.min(max, Math.max(min, Math.round(n))));
  };
  return (
    <div className="flex items-center gap-1.5">
      <Input
        inputMode="numeric"
        disabled={disabled}
        value={text}
        style={{ width }}
        className={cn(fieldCls, "num text-right")}
        onFocus={() => { focused.current = true; }}
        onChange={(e) => {
          const v = e.target.value.replace(/[^0-9]/g, "");
          setText(v);
          commit(v);
        }}
        onBlur={() => {
          focused.current = false;
          commit(text);
          const n = Number(text);
          setText(String(text.trim() === "" || !Number.isFinite(n) ? value : Math.min(max, Math.max(min, Math.round(n)))));
        }}
      />
      {suffix && <span className="text-[12px] text-muted-foreground">{suffix}</span>}
    </div>
  );
}

export function SelectField<T extends string>({
  value, options, onChange, width = 200, disabled,
}: { value: T; options: readonly (readonly [T, string])[]; onChange: (v: T) => void; width?: number; disabled?: boolean }) {
  return (
    <Select value={value} onValueChange={(v) => onChange(v as T)} disabled={disabled}>
      <SelectTrigger size="sm" style={{ width }} className="data-[size=sm]:h-[30px] text-[13px]">
        <SelectValue />
      </SelectTrigger>
      <SelectContent position="popper" align="end">
        {options.map(([v, l]) => (
          <SelectItem key={v} value={v} className="text-[13px]">{l}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function ToggleRow({
  label, hint, checked, onChange, disabled,
}: { label: string; hint?: ReactNode; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <SettingRow label={label} hint={hint}>
      <Switch checked={checked} onCheckedChange={onChange} disabled={disabled} aria-label={label} />
    </SettingRow>
  );
}

export function CheckRow({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex cursor-default items-center gap-2.5 border-b border-border/60 py-2.5 text-[13px] last:border-b-0">
      <Checkbox checked={checked} onCheckedChange={(v) => onChange(v === true)} />
      {label}
    </label>
  );
}

/** Champ texte nullable : une chaîne vide devient `null`. */
export function TextField({
  value, onChange, placeholder, width = 260, mono, validate,
}: { value: string | null; onChange: (v: string | null) => void; placeholder?: string; width?: number; mono?: boolean; validate?: (v: string) => string | null }) {
  const [text, setText] = useState(value ?? "");
  const [err, setErr] = useState<string | null>(null);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(value ?? "");
  }, [value]);
  return (
    <div className="flex flex-col items-end gap-1">
      <Input
        value={text}
        style={{ width }}
        placeholder={placeholder}
        aria-invalid={err ? true : undefined}
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        className={cn(fieldCls, mono && "num")}
        onFocus={() => { focused.current = true; }}
        onBlur={() => { focused.current = false; }}
        onChange={(e) => {
          const v = e.target.value;
          setText(v);
          const msg = v.trim() && validate ? validate(v) : null;
          setErr(msg);
          if (!msg) onChange(v.trim() === "" ? null : v);
        }}
      />
      {err && <span className="text-[12px] text-destructive">{err}</span>}
    </div>
  );
}

/** Liste de codes séparés par des virgules. */
export function ListField({ value, onChange, placeholder, width = 260 }: { value: string[]; onChange: (v: string[]) => void; placeholder?: string; width?: number }) {
  const [text, setText] = useState(value.join(", "));
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(value.join(", "));
  }, [value]);
  return (
    <Input
      value={text}
      style={{ width }}
      placeholder={placeholder}
      autoCapitalize="off"
      autoCorrect="off"
      spellCheck={false}
      className={cn(fieldCls, "num")}
      onFocus={() => { focused.current = true; }}
      onBlur={() => { focused.current = false; setText(value.join(", ")); }}
      onChange={(e) => {
        setText(e.target.value);
        onChange(e.target.value.split(/[,\s;]+/).map((x) => x.trim()).filter(Boolean));
      }}
    />
  );
}

/** Champ + bouton « Choisir… » pour un dossier ou un fichier ; la saisie est validée à la sortie du champ. */
export function PathField({
  value, onChange, directory, placeholder, width = 250,
}: { value: string | null; onChange: (v: string | null) => void; directory?: boolean; placeholder?: string; width?: number }) {
  const [text, setText] = useState(value ?? "");
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(value ?? "");
  }, [value]);
  const commit = (v: string) => {
    const t = v.trim();
    if ((t || null) !== value) onChange(t === "" ? null : t);
  };
  const choose = async () => {
    const r = await open({ directory: !!directory, multiple: false, defaultPath: value ?? undefined });
    if (typeof r === "string") { setText(r); onChange(r); }
  };
  return (
    <div className="flex items-center gap-1.5">
      <Input
        value={text}
        style={{ width }}
        placeholder={placeholder}
        spellCheck={false}
        title={value ?? undefined}
        className={cn(fieldCls, "num")}
        onFocus={() => { focused.current = true; }}
        onBlur={() => { focused.current = false; commit(text); }}
        onKeyDown={(e) => { if (e.key === "Enter") commit(text); }}
        onChange={(e) => setText(e.target.value)}
      />
      <Button variant="outline" className={btnCls} onClick={choose}>Choisir…</Button>
    </div>
  );
}
