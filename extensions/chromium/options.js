const options = { closeTab: false, takeOver: true };
for (const name of Object.keys(options)) {
  const box = document.getElementById(name);
  chrome.storage.sync.get({ [name]: options[name] }).then((v) => { box.checked = !!v[name]; });
  box.addEventListener("change", () => chrome.storage.sync.set({ [name]: box.checked }));
}
