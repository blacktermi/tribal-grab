// Tribal Grab : envoie une adresse à l'application par le schéma tribalgrab://add?url=…
// Seules les adresses http et https sont transmises ; l'application refuse tout le reste.

const MENU_ID = "send-to-tribal-grab";

function isHttp(u) {
  try {
    const p = new URL(u);
    return p.protocol === "http:" || p.protocol === "https:";
  } catch (_) {
    return false;
  }
}

async function sendToApp(url, tabId, referrer) {
  if (!isHttp(url)) {
    await flash(tabId, "!", "#b3261e", "Cette adresse ne peut pas être envoyée (http ou https seulement).");
    return;
  }
  let target = "tribalgrab://add?url=" + encodeURIComponent(url);
  if (referrer && isHttp(referrer)) target += "&referer=" + encodeURIComponent(referrer);
  if (typeof tabId === "number") {
    // Chrome affiche sa confirmation « Ouvrir Tribal Grab ? » la première fois ; la page reste en place.
    await chrome.tabs.update(tabId, { url: target });
  } else {
    const t = await chrome.tabs.create({ url: target, active: false });
    setTimeout(() => chrome.tabs.remove(t.id).catch(() => {}), 1500);
  }
  await flash(tabId, "✓", "#2e7d32", "Envoyé à Tribal Grab");
  const { closeTab } = await chrome.storage.sync.get({ closeTab: false });
  if (closeTab && typeof tabId === "number") {
    setTimeout(() => chrome.tabs.remove(tabId).catch(() => {}), 900);
  }
}

async function flash(tabId, text, color, title) {
  try {
    await chrome.action.setBadgeBackgroundColor({ color });
    await chrome.action.setBadgeText({ text, tabId });
    await chrome.action.setTitle({ title, tabId });
    setTimeout(() => {
      chrome.action.setBadgeText({ text: "", tabId }).catch(() => {});
      chrome.action.setTitle({ title: "Envoyer à Tribal Grab", tabId }).catch(() => {});
    }, 2200);
  } catch (_) {}
}

chrome.action.onClicked.addListener((tab) => {
  if (tab && tab.url) sendToApp(tab.url, tab.id);
});

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: MENU_ID,
    title: "Envoyer à Tribal Grab",
    contexts: ["link", "page", "video", "audio"],
  });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId !== MENU_ID) return;
  const url = info.linkUrl || info.srcUrl || info.pageUrl || (tab && tab.url);
  // Un clic droit sur un lien ne ferme jamais l'onglet courant : seule la page entière peut l'être.
  const isPage = !info.linkUrl && !info.srcUrl;
  sendToApp(url, isPage && tab ? tab.id : undefined);
});

// Les téléchargements de Chrome passent par Tribal Grab. Les fichiers blob: et data: n'ont pas d'adresse à
// transmettre : ils restent dans Chrome.
chrome.downloads.onCreated.addListener(async (item) => {
  const { takeOver } = await chrome.storage.sync.get({ takeOver: true });
  const url = item.finalUrl || item.url;
  if (!takeOver || !isHttp(url) || item.state !== "in_progress") return;
  try {
    await chrome.downloads.cancel(item.id);
    await chrome.downloads.erase({ id: item.id });
  } catch (_) {
    return; // Déjà terminé : Chrome garde ce téléchargement.
  }
  await sendToApp(url, undefined, item.referrer);
});
