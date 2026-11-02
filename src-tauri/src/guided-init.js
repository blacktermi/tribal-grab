// Script d'initialisation de l'extraction guidée (injecté dans la page et ses iframes).
// Il ne fait que signaler les médias repérés à `guided_report`, la seule commande permise ici.
(() => {
  if (window.__tgInstalled) return;
  window.__tgInstalled = true;
  const TOP = window.top === window;
  const MAX_PER_PAGE = 400;
  const seen = new Set();
  let count = 0;

  function invoke(report) {
    try {
      const i = window.__TAURI_INTERNALS__;
      if (i && i.invoke) i.invoke("guided_report", { report }).catch(() => {});
    } catch (_) {}
  }
  function send(report) {
    if (TOP) invoke(report);
    else {
      try { window.top.postMessage({ __tgReport: report }, "*"); } catch (_) {}
    }
  }
  function abs(u) {
    try { return new URL(u, document.baseURI).href; } catch (_) { return null; }
  }
  function report(kind, url, extra) {
    if (!url || typeof url !== "string") return;
    const a = abs(url);
    if (!a || !/^https?:/i.test(a) || a.length > 4000) return;
    const key = kind + "|" + a.split("#")[0];
    if (seen.has(key) || count >= MAX_PER_PAGE) return;
    seen.add(key);
    count++;
    send(Object.assign({ kind, url: a, page: location.href }, extra || {}));
  }

  if (TOP) {
    window.addEventListener("message", (e) => {
      const d = e && e.data;
      if (d && typeof d === "object" && d.__tgReport && typeof d.__tgReport === "object") invoke(d.__tgReport);
    });
  }
  window.addEventListener("message", (e) => {
    const d = e && e.data;
    if (d && typeof d === "object" && d.__tgMute !== undefined) setMuted(!!d.__tgMute, false);
  });

  // fetch
  if (window.fetch) {
    const orig = window.fetch;
    window.fetch = function (input, init) {
      let reqUrl = null;
      try { reqUrl = typeof input === "string" ? input : input && input.url; } catch (_) {}
      const p = orig.apply(this, arguments);
      try {
        p.then((res) => {
          try { report("fetch", res.url || reqUrl, { mime: res.headers.get("content-type") || undefined, size: Number(res.headers.get("content-length")) || undefined }); } catch (_) {}
        }).catch(() => {});
      } catch (_) {}
      return p;
    };
  }
  // XMLHttpRequest
  if (window.XMLHttpRequest) {
    const open = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function (method, url) {
      try {
        this.__tgUrl = url;
        this.addEventListener("readystatechange", () => {
          if (this.readyState === 2) {
            try { report("xhr", this.responseURL || this.__tgUrl, { mime: this.getResponseHeader("content-type") || undefined, size: Number(this.getResponseHeader("content-length")) || undefined }); } catch (_) {}
          }
        });
      } catch (_) {}
      return open.apply(this, arguments);
    };
  }

  // Éléments video, audio, source, img
  function scanEl(el) {
    try {
      const tag = el.tagName;
      if (tag === "VIDEO" || tag === "AUDIO") {
        const kind = tag === "VIDEO" ? "video" : "audio";
        if (el.currentSrc) report(kind, el.currentSrc, { width: el.videoWidth || undefined, height: el.videoHeight || undefined });
        if (el.src) report(kind, el.src);
        if (el.poster) report("image", el.poster);
        if (window.__tgMuted) el.muted = true;
      } else if (tag === "SOURCE") {
        const parent = el.parentElement;
        const kind = parent && parent.tagName === "AUDIO" ? "audio" : "source";
        if (el.src) report(kind, el.src, { mime: el.type || undefined });
      } else if (tag === "IMG") {
        const add = () => {
          const w = el.naturalWidth, h = el.naturalHeight;
          if (w >= 120 && h >= 120) report("image", el.currentSrc || el.src, { width: w, height: h });
        };
        if (el.complete) add(); else el.addEventListener("load", add, { once: true });
      }
    } catch (_) {}
  }
  function scanTree(root) {
    try {
      if (root.nodeType !== 1) return;
      scanEl(root);
      root.querySelectorAll && root.querySelectorAll("video,audio,source,img").forEach(scanEl);
    } catch (_) {}
  }
  function scanMeta() {
    try {
      document.querySelectorAll('meta[property="og:video"],meta[property="og:video:url"],meta[property="og:video:secure_url"],meta[property="og:audio"]').forEach((m) => report("video", m.content));
      const ogi = document.querySelector('meta[property="og:image"]');
      if (ogi) report("image", ogi.content);
    } catch (_) {}
  }
  document.addEventListener("loadstart", (e) => scanEl(e.target), true);
  document.addEventListener("loadedmetadata", (e) => scanEl(e.target), true);
  document.addEventListener("play", (e) => { scanEl(e.target); if (window.__tgMuted && e.target && e.target.muted === false) e.target.muted = true; }, true);

  function start() {
    scanTree(document.documentElement);
    scanMeta();
    try {
      new MutationObserver((muts) => {
        for (const m of muts) {
          m.addedNodes && m.addedNodes.forEach(scanTree);
          if (m.type === "attributes") scanEl(m.target);
        }
      }).observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ["src", "poster"] });
    } catch (_) {}
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start, { once: true });
  else start();

  // Ressources réseau (couvre aussi les requêtes faites par des lecteurs natifs ou des workers)
  try {
    const handle = (e) => {
      const t = e.initiatorType;
      if (t === "img" || t === "image") return;
      const u = e.name;
      if (/\.(m3u8|mpd|mp4|webm|m4v|mov|mp3|m4a|ogg|opus|flac|wav)(\?|#|$)/i.test(u) || /\/(manifest|playlist|master)\b/i.test(u)) report("resource", u);
    };
    performance.getEntriesByType("resource").forEach(handle);
    new PerformanceObserver((l) => l.getEntries().forEach(handle)).observe({ type: "resource", buffered: true });
  } catch (_) {}

  // Titre et adresse (haut de page uniquement)
  function pageInfo() {
    if (!TOP) return;
    send({ kind: "page", url: location.href, title: document.title || "" });
  }
  if (TOP) {
    window.addEventListener("load", pageInfo);
    try { new MutationObserver(pageInfo).observe(document.querySelector("title") || document.documentElement, { childList: true, subtree: true, characterData: true }); } catch (_) {}
    setTimeout(pageInfo, 800);
  }

  // Son coupé
  function setMuted(m, broadcast) {
    window.__tgMuted = m;
    document.querySelectorAll("video,audio").forEach((v) => { v.muted = m; });
    if (broadcast) {
      for (let i = 0; i < window.frames.length; i++) {
        try { window.frames[i].postMessage({ __tgMute: m }, "*"); } catch (_) {}
      }
    }
  }
  window.__tgSetMuted = (m) => setMuted(!!m, true);
})();
