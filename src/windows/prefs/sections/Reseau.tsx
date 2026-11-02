import { useEffect, useRef, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Lock } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { SettingGroup, SettingRow } from "@/components/app/Setting";
import { attempt } from "@/components/app/errors";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { btnCls, fieldCls, NumField, Page, PathField, SelectField, TextField, ToggleRow, useS } from "./controls";
import ReseauLogin from "./ReseauLogin";

const schema = z.object({
  url: z.string().trim().refine((v) => v === "" || /^(https?|socks4|socks5h?):\/\/[^\s/]+(:\d+)?\/?$/i.test(v), "Adresse attendue : http://hôte:port ou socks5://hôte:port."),
  username: z.string().trim(),
});
type Form = z.infer<typeof schema>;

const BROWSERS: readonly (readonly [string, string])[] = [
  ["none", "Aucun"], ["safari", "Safari"], ["chrome", "Chrome"], ["firefox", "Firefox"], ["brave", "Brave"],
  ["edge", "Edge"], ["chromium", "Chromium"], ["vivaldi", "Vivaldi"], ["opera", "Opera"], ["arc", "Arc"],
];

const IP = /^(\d{1,3}(\.\d{1,3}){3}|[0-9a-fA-F:]+:[0-9a-fA-F:]*)$/;

function ProxyBlock() {
  const { s, patch } = useS();
  const { register, trigger, getValues, setValue, formState: { errors } } = useForm<Form>({
    resolver: zodResolver(schema),
    defaultValues: { url: s.proxy?.url ?? "", username: s.proxy?.username ?? "" },
    mode: "onChange",
  });
  const [pwd, setPwd] = useState("");
  const [busy, setBusy] = useState(false);
  const pwdRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const active = document.activeElement?.getAttribute("name");
    if (active !== "url" && getValues("url") !== (s.proxy?.url ?? "")) setValue("url", s.proxy?.url ?? "");
    if (active !== "username" && getValues("username") !== (s.proxy?.username ?? "")) setValue("username", s.proxy?.username ?? "");
  }, [s.proxy, getValues, setValue]);

  const apply = async () => {
    if (!(await trigger())) return;
    const v = getValues();
    const url = v.url.trim();
    patch({ proxy: url === "" ? null : { url, username: v.username.trim() === "" ? null : v.username.trim() } });
  };
  const url = register("url", { onChange: apply });
  const username = register("username", { onChange: apply });

  const savePassword = async () => {
    if (pwd === "" || busy) return;
    setBusy(true);
    const ok = await attempt(ipc.setProxyPassword(pwd), "Mot de passe enregistré dans le trousseau.");
    setBusy(false);
    if (ok !== undefined) setPwd("");
  };
  const clearPassword = async () => {
    await attempt(ipc.setProxyPassword(""), "Mot de passe effacé.");
    setPwd("");
  };

  return (
    <SettingGroup title="Proxy">
      <SettingRow label="Adresse du proxy" hint="Laisser vide pour une connexion directe.">
        <div className="flex flex-col items-end gap-1">
          <Input {...url} placeholder="http://127.0.0.1:8080" spellCheck={false} autoCapitalize="off" style={{ width: 260 }}
            aria-invalid={errors.url ? true : undefined} className={cn(fieldCls, "num")} />
          {errors.url && <span className="max-w-[260px] text-right text-[12px] text-destructive">{errors.url.message}</span>}
        </div>
      </SettingRow>
      <SettingRow label="Nom d'utilisateur">
        <Input {...username} spellCheck={false} autoCapitalize="off" autoComplete="off" style={{ width: 260 }} className={fieldCls} />
      </SettingRow>
      <SettingRow
        label="Mot de passe"
        hint={<span className="inline-flex items-center gap-1"><Lock className="size-3" />Stocké dans le trousseau macOS, jamais affiché.</span>}
      >
        <Input
          ref={pwdRef}
          type="password"
          value={pwd}
          autoComplete="new-password"
          placeholder="Saisir pour remplacer"
          style={{ width: 170 }}
          className={fieldCls}
          onChange={(e) => setPwd(e.target.value)}
          onBlur={savePassword}
          onKeyDown={(e) => { if (e.key === "Enter") pwdRef.current?.blur(); }}
        />
        <Button variant="outline" className={btnCls} onClick={clearPassword}>Effacer</Button>
      </SettingRow>
    </SettingGroup>
  );
}

export default function Reseau() {
  const { s, patch } = useS();
  return (
    <Page>
      <ProxyBlock />

      <SettingGroup title="Requêtes">
        <SettingRow label="Identifiant du navigateur (User-Agent)" hint="Vide : valeur par défaut.">
          <TextField value={s.user_agent} width={260} mono onChange={(v) => patch({ user_agent: v })} />
        </SettingRow>
        <SettingRow label="Délai d'attente" hint="Durée sans réponse avant d'abandonner une requête.">
          <NumField value={s.timeout_secs} min={1} max={600} suffix="s" onChange={(n) => patch({ timeout_secs: n })} />
        </SettingRow>
        <ToggleRow label="Fragments en parallèle" hint="Accélère les flux HLS et DASH." checked={s.fragmented} onChange={(v) => patch({ fragmented: v })} />
        <SettingRow label="Adresse IP pour les vidéos limitées à un pays" hint="Envoyée dans l'en-tête X-Forwarded-For.">
          <TextField value={s.xff} width={200} mono placeholder="203.0.113.7"
            validate={(v) => (IP.test(v.trim()) ? null : "Adresse IP invalide.")} onChange={(v) => patch({ xff: v })} />
        </SettingRow>
      </SettingGroup>

      <SettingGroup title="Cookies">
        <SettingRow label="Cookies d'un navigateur" hint="Pour les contenus qui demandent un compte.">
          <SelectField value={s.cookies_browser ?? "none"} options={BROWSERS} width={160}
            onChange={(v) => patch({ cookies_browser: v === "none" ? null : v })} />
        </SettingRow>
        <SettingRow label="Fichier de cookies" hint="Format Netscape (cookies.txt).">
          <PathField value={s.cookies_file} width={200} onChange={(v) => patch({ cookies_file: v })} />
        </SettingRow>
      </SettingGroup>

      <ReseauLogin />
    </Page>
  );
}
