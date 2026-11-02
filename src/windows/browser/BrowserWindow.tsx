import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowRight, RotateCw, Star, Volume2, VolumeX, Send, Image as ImageIcon, Film, Music, Radio, Bookmark, History, KeyRound, Loader2, X, Download } from "lucide-react";
import { toast } from "sonner";
import { Titlebar } from "@/components/app/Titlebar";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { cn } from "@/lib/utils";
import { ipc, errMsg } from "@/lib/ipc";
import { on, subscribe } from "@/app/events";
import { bytes, dateTime, hostOf } from "@/lib/format";
import type { Bookmark as Bm, BrowserHistoryEntry, Detected, LoginStatus } from "@/types";

type Tab = "media" | "images" | "bookmarks" | "history" | "login";

function nameOf(u: string): string {
  try {
    const p = new URL(u).pathname.split("/").filter(Boolean);
    return decodeURIComponent(p[p.length - 1] ?? new URL(u).hostname);
  } catch { return u; }
}

const KIND: Record<Detected["kind"], { label: string; icon: typeof Film }> = {
  video: { label: "Vidéo", icon: Film },
  audio: { label: "Audio", icon: Music },
  hls: { label: "Flux HLS", icon: Radio },
  dash: { label: "Flux DASH", icon: Radio },
  image: { label: "Image", icon: ImageIcon },
};

export default function BrowserWindow() {
  const initialMode = new URLSearchParams(window.location.search).get("mode") === "login" ? "login" : "guided";
  const [mode, setMode] = useState<"guided" | "login">(initialMode);
  const [address, setAddress] = useState("");
  const [pageUrl, setPageUrl] = useState("");
  const [title, setTitle] = useState("");
  const [loading, setLoading] = useState(false);
  const [hasPage, setHasPage] = useState(false);
  const [muted, setMuted] = useState(false);
  const [media, setMedia] = useState<Detected[]>([]);
  const [bookmarks, setBookmarks] = useState<Bm[]>([]);
  const [history, setHistory] = useState<BrowserHistoryEntry[]>([]);
  const [tab, setTab] = useState<Tab>(initialMode === "login" ? "login" : "media");
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [login, setLogin] = useState<LoginStatus | null>(null);
  const hole = useRef<HTMLDivElement>(null);
  const editing = useRef(false);

  const refresh = useCallback(async () => {
    const s = await ipc.browserState();
    setMedia(s.media); setPageUrl(s.page_url); setTitle(s.page_title); setMuted(s.muted);
    setBookmarks(s.bookmarks); setHistory(s.history); setHasPage(s.has_page);
    if (!editing.current) setAddress(s.page_url);
    if (s.mode === "login" || s.mode === "guided") { setMode(s.mode); }
  }, []);

  useEffect(() => {
    refresh();
    ipc.loginStatus().then(setLogin).catch(() => {});
    return subscribe(
      on<Detected>("guided://media", (d) => setMedia((m) => (m.some((x) => x.id === d.id) ? m : [...m, d]))),
      on<string>("guided://reset", (u) => { setMedia([]); setPicked(new Set()); setPageUrl(u); setTitle(""); setLoading(true); if (!editing.current) setAddress(u); setHasPage(true); }),
      on<string>("guided://loading", (u) => { setLoading(true); setPageUrl(u); if (!editing.current) setAddress(u); setHasPage(true); }),
      on<string>("guided://loaded", () => { setLoading(false); refresh(); }),
      on<{ url: string; title: string }>("guided://page", (p) => { setPageUrl(p.url); setTitle(p.title); if (!editing.current) setAddress(p.url); }),
      on<string>("guided://mode", (m) => { const mm = m === "login" ? "login" : "guided"; setMode(mm); setTab(mm === "login" ? "login" : "media"); }),
    );
  }, [refresh]);

  // La page web est une vue native posée sur ce rectangle : l'interface lui communique sa position.
  useLayoutEffect(() => {
    const el = hole.current;
    if (!el) return;
    let raf = 0;
    const report = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const r = el.getBoundingClientRect();
        ipc.guidedLayout(r.left, r.top, r.width, r.height).catch(() => {});
      });
    };
    const ro = new ResizeObserver(report);
    ro.observe(el);
    window.addEventListener("resize", report);
    report();
    return () => { ro.disconnect(); window.removeEventListener("resize", report); cancelAnimationFrame(raf); };
  }, []);

  async function go(e?: React.FormEvent) {
    e?.preventDefault();
    editing.current = false;
    try { await ipc.guidedNavigate(address); setHasPage(true); } catch (err) { toast.error(errMsg(err)); }
  }
  const bookmarked = bookmarks.some((b) => b.url === pageUrl);
  const toggleBookmark = async () => {
    if (!pageUrl) return;
    try { await ipc.bookmarkToggle(pageUrl, title || hostOf(pageUrl)); await refresh(); } catch (e) { toast.error(errMsg(e)); }
  };
  async function send(urls: string[]) {
    try {
      const ids = await ipc.guidedSend(urls, pageUrl || null);
      toast.success(ids.length === 1 ? "Envoyé à la file" : `${ids.length} éléments envoyés à la file`);
    } catch (e) { toast.error(errMsg(e)); }
  }

  const videos = media.filter((m) => m.kind !== "image");
  const images = media.filter((m) => m.kind === "image");
  const tabs: { id: Tab; label: string; icon: typeof Film; n?: number }[] = mode === "login"
    ? [{ id: "login", label: "Connexion", icon: KeyRound }, { id: "bookmarks", label: "Marque-pages", icon: Bookmark }, { id: "history", label: "Historique", icon: History }]
    : [{ id: "media", label: "Médias", icon: Film, n: videos.length }, { id: "images", label: "Images", icon: ImageIcon, n: images.length }, { id: "bookmarks", label: "Marque-pages", icon: Bookmark }, { id: "history", label: "Historique", icon: History }];

  const list = tab === "media" ? videos : images;
  const togglePick = (id: number) => setPicked((s) => { const x = new Set(s); if (x.has(id)) x.delete(id); else x.add(id); return x; });
  const pickedItems = list.filter((m) => picked.has(m.id));

  return (
    <div className="flex h-full flex-col bg-background">
      <Titlebar className="gap-1.5">
        <div className="no-drag flex items-center gap-0.5">
          <Button size="icon-sm" variant="ghost" className="size-7" title="Précédent" onClick={() => ipc.guidedNav("back")}><ArrowLeft className="size-4" /></Button>
          <Button size="icon-sm" variant="ghost" className="size-7" title="Suivant" onClick={() => ipc.guidedNav("forward")}><ArrowRight className="size-4" /></Button>
          <Button size="icon-sm" variant="ghost" className="size-7" title={loading ? "Arrêter" : "Recharger"} onClick={() => ipc.guidedNav(loading ? "stop" : "reload")}>{loading ? <X className="size-4" /> : <RotateCw className="size-4" />}</Button>
        </div>
        <form onSubmit={go} className="no-drag relative flex min-w-0 flex-1 items-center">
          {loading && <Loader2 className="pointer-events-none absolute left-2.5 size-3.5 animate-spin text-primary" />}
          <input
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            onFocus={(e) => { editing.current = true; e.currentTarget.select(); }}
            onBlur={() => { editing.current = false; }}
            placeholder="Adresse, ou texte à rechercher"
            spellCheck={false}
            className={cn("h-8 w-full rounded-md border border-input bg-card pr-3 text-[13px] outline-none focus:border-ring focus:ring-2 focus:ring-ring/30", loading ? "pl-8" : "pl-3")}
          />
        </form>
        <div className="no-drag flex items-center gap-0.5">
          <Button size="icon-sm" variant="ghost" className="size-7" title={bookmarked ? "Retirer des marque-pages" : "Ajouter aux marque-pages"} onClick={toggleBookmark}><Star className={cn("size-4", bookmarked && "fill-primary text-primary")} /></Button>
          <Button size="icon-sm" variant="ghost" className="size-7" title={muted ? "Rétablir le son" : "Couper le son"} onClick={() => { setMuted(!muted); ipc.guidedMute(!muted); }}>{muted ? <VolumeX className="size-4 text-primary" /> : <Volume2 className="size-4" />}</Button>
          {mode === "guided" && <Button size="sm" variant="secondary" className="ml-1 h-7" disabled={!pageUrl} title="Envoyer l'adresse de cette page à la file : Tribal Grab l'analysera" onClick={() => send([pageUrl])}><Send className="size-3.5" />Envoyer la page</Button>}
        </div>
      </Titlebar>

      <div className="flex min-h-0 flex-1">
        <div ref={hole} className="relative min-w-0 flex-1 bg-background">
          {!hasPage && (
            <div className="flex h-full items-center justify-center px-8">
              <div className="max-w-[440px]">
                <h2 className="text-[15px] font-semibold">{mode === "login" ? "Connectez-vous à un site" : "Parcourez la page, Tribal Grab repère les médias"}</h2>
                <p className="mt-1 text-[13px] leading-relaxed text-muted-foreground">
                  {mode === "login"
                    ? "Entrez l'adresse du site, connectez-vous comme dans n'importe quel navigateur, puis enregistrez la connexion : Tribal Grab et yt-dlp s'en serviront pour les vidéos réservées aux comptes."
                    : "Entrez une adresse en haut. Lancez la lecture d'une vidéo : les flux et fichiers détectés s'affichent à droite, prêts à être envoyés à la file."}
                </p>
                {bookmarks.length > 0 && (
                  <div className="mt-5">
                    <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">Marque-pages</div>
                    {bookmarks.slice(0, 6).map((b) => (
                      <button key={b.url} className="block w-full truncate rounded px-2 py-1 text-left text-[13px] hover:bg-accent" onClick={() => { setAddress(b.url); ipc.guidedNavigate(b.url).then(() => setHasPage(true)); }}>{b.title || b.url}</button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        <aside className="flex w-[310px] shrink-0 flex-col border-l border-border bg-surface">
          <div className="flex shrink-0 items-center gap-0.5 border-b border-border px-2 py-1.5">
            {tabs.map((t) => (
              <button key={t.id} onClick={() => { setTab(t.id); setPicked(new Set()); }} title={t.label} className={cn("flex h-7 items-center gap-1.5 rounded-md px-2 text-[12px]", tab === t.id ? "bg-accent text-foreground" : "text-muted-foreground hover:bg-accent/60")}>
                <t.icon className="size-3.5" />{tab === t.id && t.label}
                {t.n != null && t.n > 0 && <span className="num rounded-full bg-primary px-1.5 text-[10px] font-semibold text-primary-foreground">{t.n}</span>}
              </button>
            ))}
          </div>

          {(tab === "media" || tab === "images") && (
            <>
              <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-1.5 text-[12px] text-muted-foreground">
                <span>{list.length} {tab === "media" ? "média" : "image"}{list.length > 1 ? "s" : ""} détecté{list.length > 1 ? "s" : ""}</span>
                <span className="flex-1" />
                {list.length > 0 && <button className="hover:text-foreground" onClick={() => setPicked(picked.size === list.length ? new Set() : new Set(list.map((m) => m.id)))}>{picked.size === list.length ? "Aucun" : "Tout"}</button>}
                <button className="hover:text-foreground" onClick={() => { ipc.guidedClearMedia(); setMedia([]); setPicked(new Set()); }}>Vider</button>
              </div>
              <div className="min-h-0 flex-1 overflow-y-auto">
                {list.length === 0 && (
                  <div className="p-4 text-[12.5px] leading-relaxed text-muted-foreground">
                    {tab === "media" ? "Aucun média pour l'instant. Lancez la lecture d'une vidéo sur la page : les flux apparaissent ici." : "Aucune image assez grande n'a été repérée sur cette page."}
                  </div>
                )}
                {tab === "media" && list.map((m) => {
                  const K = KIND[m.kind];
                  return (
                    <div key={m.id} className="flex items-start gap-2 border-b border-border/60 px-3 py-2 hover:bg-row-hover">
                      <Checkbox className="mt-0.5" checked={picked.has(m.id)} onCheckedChange={() => togglePick(m.id)} />
                      <K.icon className="mt-0.5 size-4 shrink-0 text-muted-foreground" strokeWidth={1.6} />
                      <div className="min-w-0 flex-1">
                        <div className="selectable truncate text-[12.5px] font-medium" title={m.url}>{nameOf(m.url)}</div>
                        <div className="num truncate text-[11px] text-muted-foreground">{K.label}{m.height ? ` · ${m.height}p` : ""}{m.size ? ` · ${bytes(m.size)}` : ""}{m.mime ? ` · ${m.mime.split("/").pop()}` : ""} · {hostOf(m.url)}</div>
                      </div>
                      <button className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground" title="Envoyer à la file" onClick={() => send([m.url])}><Download className="size-3.5" /></button>
                    </div>
                  );
                })}
                {tab === "images" && (
                  <div className="grid grid-cols-2 gap-2 p-2">
                    {list.map((m) => (
                      <div key={m.id} className={cn("group relative overflow-hidden rounded-md border bg-muted", picked.has(m.id) ? "border-primary" : "border-border")}>
                        <button className="block aspect-square w-full" onClick={() => togglePick(m.id)}>
                          <img src={m.url} alt="" loading="lazy" referrerPolicy="no-referrer" draggable={false} className="size-full object-cover" />
                        </button>
                        <div className="num absolute inset-x-0 bottom-0 bg-background/80 px-1.5 py-0.5 text-[10px] text-muted-foreground">{m.width && m.height ? `${m.width}×${m.height}` : hostOf(m.url)}</div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <div className="shrink-0 border-t border-border p-2">
                <Button className="w-full" size="sm" disabled={pickedItems.length === 0} onClick={() => send(pickedItems.map((m) => m.url))}>
                  <Send className="size-3.5" />{pickedItems.length > 0 ? `Envoyer ${pickedItems.length} à la file` : "Sélectionnez des éléments"}
                </Button>
              </div>
            </>
          )}

          {tab === "bookmarks" && (
            <div className="min-h-0 flex-1 overflow-y-auto">
              {bookmarks.length === 0 && <div className="p-4 text-[12.5px] text-muted-foreground">Aucun marque-page. Cliquez sur l'étoile pour garder la page actuelle.</div>}
              {bookmarks.map((b) => (
                <div key={b.url} className="group flex items-center gap-2 border-b border-border/60 px-3 py-2 hover:bg-row-hover">
                  <button className="min-w-0 flex-1 text-left" onClick={() => { setAddress(b.url); ipc.guidedNavigate(b.url).then(() => setHasPage(true)); }}>
                    <div className="truncate text-[12.5px] font-medium">{b.title || b.url}</div>
                    <div className="truncate text-[11px] text-muted-foreground">{hostOf(b.url)}</div>
                  </button>
                  <button className="rounded p-1 text-muted-foreground opacity-0 hover:bg-accent group-hover:opacity-100" title="Retirer" onClick={async () => { await ipc.bookmarkToggle(b.url, b.title); refresh(); }}><X className="size-3.5" /></button>
                </div>
              ))}
            </div>
          )}

          {tab === "history" && (
            <>
              <div className="flex shrink-0 items-center border-b border-border px-3 py-1.5 text-[12px] text-muted-foreground"><span>{history.length} page{history.length > 1 ? "s" : ""}</span><span className="flex-1" /><button className="hover:text-foreground" onClick={async () => { await ipc.guidedHistoryClear(); refresh(); }}>Effacer</button></div>
              <div className="min-h-0 flex-1 overflow-y-auto">
                {history.map((h, i) => (
                  <button key={`${h.at}-${i}`} className="block w-full border-b border-border/60 px-3 py-2 text-left hover:bg-row-hover" onClick={() => { setAddress(h.url); ipc.guidedNavigate(h.url).then(() => setHasPage(true)); }}>
                    <div className="truncate text-[12.5px] font-medium">{h.title || h.url}</div>
                    <div className="num truncate text-[11px] text-muted-foreground">{hostOf(h.url)} · {dateTime(h.at)}</div>
                  </button>
                ))}
                {history.length === 0 && <div className="p-4 text-[12.5px] text-muted-foreground">L'historique de navigation est vide.</div>}
              </div>
            </>
          )}

          {tab === "login" && (
            <div className="min-h-0 flex-1 space-y-3 overflow-y-auto p-4 text-[12.5px] leading-relaxed">
              <p className="text-muted-foreground">Une fois connecté, enregistrez la session : ses cookies sont écrits dans un fichier au format Netscape, que le moteur et yt-dlp utilisent pour les contenus réservés aux comptes. Ils restent sur ce Mac.</p>
              <Button className="w-full" onClick={async () => { try { const n = await ipc.loginSave(); setLogin(await ipc.loginStatus()); toast.success(`Connexion enregistrée (${n} cookies)`); } catch (e) { toast.error(errMsg(e)); } }}>Enregistrer la connexion</Button>
              <div className="rounded-md border border-border bg-card p-3 text-[12px]">
                <div className="flex justify-between"><span className="text-muted-foreground">Cookies enregistrés</span><span className="num">{login?.cookies ?? 0}</span></div>
                <div className="mt-1 flex justify-between"><span className="text-muted-foreground">Utilisés par le moteur</span><span>{login?.active ? "Oui" : "Non"}</span></div>
                {login?.updated_at ? <div className="mt-1 flex justify-between"><span className="text-muted-foreground">Mis à jour</span><span className="num">{dateTime(login.updated_at)}</span></div> : null}
              </div>
              <p className="text-muted-foreground">La fermeture de cette fenêtre enregistre aussi la session. La gestion des données par site se trouve dans Préférences, Réseau.</p>
              <Button variant="ghost" size="sm" onClick={() => ipc.openWindow("prefs", "reseau")}>Ouvrir les préférences réseau</Button>
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
