/* ==========================================================================
   Beacon Website — Interactive Controls, Screenshot Showcase & Release Fetcher
   ========================================================================== */

const SCREENS = [
  {
    id: "dashboard",
    label: "Home",
    badge: "One-Click Server & P2P Code",
    src: "./assets/screenshots/dashboard.png",
    title: "Home — Start your world and invite friends in seconds",
    desc: "Pick a world, press Start, and share your 3-word room code (like crimson-dragon-42). Quick buttons let you make it day, clear weather, save immediately, or check who is online.",
  },
  {
    id: "mods",
    label: "Mods & Modpacks",
    badge: "Modrinth + Auto P2P Sync",
    src: "./assets/screenshots/mods.png",
    title: "Mods — Search Modrinth and sync to friends automatically",
    desc: "Browse and install Fabric/NeoForge mods, Paper plugins, and .mrpack modpacks in one click. When friends join your room code, Beacon automatically syncs the exact same mods to their PC.",
  },
  {
    id: "players",
    label: "Players",
    badge: "1-Click Admin Actions",
    src: "./assets/screenshots/players.png",
    title: "Players — Help out friends without memorizing server commands",
    desc: "Switch game modes (Survival, Creative, Spectator), teleport friends, heal and feed them, grant admin (OP), or kick/ban troublemakers with instant visual controls.",
  },
  {
    id: "inventory",
    label: "Live Inventory",
    badge: "Real-Time NBT Inspector",
    src: "./assets/screenshots/inventory.png",
    title: "Live Inventory Inspector — View and edit player items in real time",
    desc: "Inspect any online or offline player's armor, off-hand, hotbar, and 36-slot inventory directly from world/playerdata/<uuid>.dat. Click any slot to remove an item or gift items from the searchable catalog.",
  },
  {
    id: "worlds",
    label: "Worlds & Backups",
    badge: "Trusted Co-Host Failover",
    src: "./assets/screenshots/worlds.png",
    title: "Worlds — Multi-world profiles, 10-minute backups & Co-Host failover",
    desc: "Each world keeps its own loader (Paper, Fabric, NeoForge), Minecraft version, and mods. Compressed .tar.zst backups replicate to trusted friends so they can take over hosting if your PC goes offline.",
  },
  {
    id: "monitor",
    label: "Monitor & Console",
    badge: "Live Telemetry & Logs",
    src: "./assets/screenshots/monitor.png",
    title: "Monitor — Real-time TPS, memory, CPU charts & color-coded console",
    desc: "Keep an eye on server speed (20 TPS target), allocated Java memory, processor load, and live server logs with instant filtering for warnings and errors.",
  },
  {
    id: "settings",
    label: "Settings & MOTD",
    badge: "Visual Rules & MOTD Preview",
    src: "./assets/screenshots/settings.png",
    title: "Settings — Visual game rules, live MOTD editor & Java 21 tuning",
    desc: "Toggle Keep Inventory, Cracked/Offline Mode, PvP, and Whitelist. Customize your two-line Minecraft server list welcome message with live color previews and automatic RAM tuning.",
  },
];

const HERO_STATES = {
  running: {
    title: "Your world is open",
    sub: "“Survival with friends” · Friends can join with your code below.",
    dotClass: "status-dot",
    dotLabel: "World is open · 3 players online",
  },
  starting: {
    title: "Getting your world ready…",
    sub: "Provisioning Portable Java 21 & Generational ZGC flags.",
    dotClass: "status-dot status-dot--sun",
    dotLabel: "Starting server…",
  },
  stopped: {
    title: "Ready when you are",
    sub: "Pick a world and press Start. Beacon handles the rest.",
    dotClass: "status-dot status-dot--off",
    dotLabel: "World is closed",
  },
};

document.addEventListener("DOMContentLoaded", () => {
  initHeroStateDemo();
  initRoomCodeCopy();
  initShowcase();
  initLightbox();
  initCopyCommands();
  fetchLatestRelease();
});

/* ---------- 1. Hero Beacon Art State Switcher ---------- */
function initHeroStateDemo() {
  const artEl = document.getElementById("hero-beacon-art");
  const titleEl = document.getElementById("hero-live-title");
  const subEl = document.getElementById("hero-live-sub");
  const dotEl = document.getElementById("hero-status-dot");
  const dotTextEl = document.getElementById("hero-status-text");
  const buttons = document.querySelectorAll("[data-beacon-state]");

  if (!artEl || !titleEl || !subEl) return;

  buttons.forEach((btn) => {
    btn.addEventListener("click", () => {
      const state = btn.getAttribute("data-beacon-state");
      const info = HERO_STATES[state];
      if (!info) return;

      artEl.setAttribute("data-state", state);
      titleEl.textContent = info.title;
      subEl.textContent = info.sub;
      if (dotEl) dotEl.className = info.dotClass;
      if (dotTextEl) dotTextEl.textContent = info.dotLabel;

      buttons.forEach((b) => {
        const active = b === btn;
        b.classList.toggle("is-active", active);
        b.setAttribute("aria-pressed", String(active));
      });
    });
  });
}

/* ---------- 2. Demo Room Code Copy ---------- */
function initRoomCodeCopy() {
  const copyBtn = document.getElementById("copy-room-code-btn");
  const copyLabel = document.getElementById("copy-room-code-label");
  if (!copyBtn || !copyLabel) return;

  copyBtn.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText("crimson-dragon-42");
    } catch {
      // Fallback if clipboard blocked on file://
    }
    copyLabel.textContent = "Copied!";
    setTimeout(() => {
      copyLabel.textContent = "Copy code";
    }, 1800);
  });
}

/* ---------- 3. Interactive Screenshot Showcase ---------- */
let currentScreenIdx = 0;

function initShowcase() {
  const tabButtons = document.querySelectorAll("[data-screen-tab]");
  const imgEl = document.getElementById("showcase-img");
  const titleEl = document.getElementById("showcase-title");
  const descEl = document.getElementById("showcase-desc");
  const badgeEl = document.getElementById("showcase-badge");
  const winTitleEl = document.getElementById("showcase-window-title");

  if (!imgEl || !titleEl || !descEl) return;

  function selectScreen(index) {
    currentScreenIdx = (index + SCREENS.length) % SCREENS.length;
    const item = SCREENS[currentScreenIdx];

    imgEl.style.opacity = "0.35";
    setTimeout(() => {
      imgEl.src = item.src;
      imgEl.alt = item.title;
      imgEl.style.opacity = "1";
    }, 90);

    titleEl.textContent = item.title;
    descEl.textContent = item.desc;
    if (badgeEl) badgeEl.textContent = item.badge;
    if (winTitleEl) winTitleEl.textContent = `Beacon — ${item.label}`;

    tabButtons.forEach((btn, idx) => {
      const active = idx === currentScreenIdx;
      btn.classList.toggle("is-active", active);
      btn.setAttribute("aria-selected", String(active));
    });
  }

  tabButtons.forEach((btn, idx) => {
    btn.addEventListener("click", () => selectScreen(idx));
  });
}

/* ---------- 4. Full-Resolution Screenshot Lightbox ---------- */
function initLightbox() {
  const lightbox = document.getElementById("lightbox");
  const lbImg = document.getElementById("lightbox-img");
  const lbTitle = document.getElementById("lightbox-title");
  const lbClose = document.getElementById("lightbox-close");
  const lbPrev = document.getElementById("lightbox-prev");
  const lbNext = document.getElementById("lightbox-next");
  const showcaseFrame = document.getElementById("showcase-frame");
  const galleryCards = document.querySelectorAll("[data-gallery-idx]");

  if (!lightbox || !lbImg || !lbTitle) return;

  let lbIdx = 0;

  function openLightbox(index) {
    lbIdx = (index + SCREENS.length) % SCREENS.length;
    const item = SCREENS[lbIdx];
    lbImg.src = item.src;
    lbImg.alt = item.title;
    lbTitle.textContent = item.title;
    lightbox.classList.add("is-open");
    lightbox.setAttribute("aria-hidden", "false");
    document.body.style.overflow = "hidden";
  }

  function closeLightbox() {
    lightbox.classList.remove("is-open");
    lightbox.setAttribute("aria-hidden", "true");
    document.body.style.overflow = "";
  }

  if (showcaseFrame) {
    showcaseFrame.addEventListener("click", () => openLightbox(currentScreenIdx));
  }

  galleryCards.forEach((card) => {
    card.addEventListener("click", () => {
      const idx = Number(card.getAttribute("data-gallery-idx") || "0");
      openLightbox(idx);
    });
    card.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        const idx = Number(card.getAttribute("data-gallery-idx") || "0");
        openLightbox(idx);
      }
    });
  });

  if (lbClose) lbClose.addEventListener("click", closeLightbox);
  if (lbPrev) lbPrev.addEventListener("click", () => openLightbox(lbIdx - 1));
  if (lbNext) lbNext.addEventListener("click", () => openLightbox(lbIdx + 1));

  lightbox.addEventListener("click", (e) => {
    if (e.target === lightbox) closeLightbox();
  });

  window.addEventListener("keydown", (e) => {
    if (!lightbox.classList.contains("is-open")) return;
    if (e.key === "Escape") closeLightbox();
    if (e.key === "ArrowLeft") openLightbox(lbIdx - 1);
    if (e.key === "ArrowRight") openLightbox(lbIdx + 1);
  });
}

/* ---------- 5. Copy Terminal Commands ---------- */
function initCopyCommands() {
  const copyCmdBtn = document.getElementById("copy-build-cmds");
  if (!copyCmdBtn) return;

  const commands = [
    "git clone https://github.com/Sezergumus/beacon.git",
    "cd beacon",
    "npm install",
    "npm run tauri dev",
  ].join("\n");

  copyCmdBtn.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(commands);
    } catch {
      // ignore
    }
    const prev = copyCmdBtn.textContent;
    copyCmdBtn.textContent = "Copied!";
    setTimeout(() => {
      copyCmdBtn.textContent = prev;
    }, 1800);
  });
}

/* ---------- 6. Dynamic GitHub Release Info ---------- */
async function fetchLatestRelease() {
  const versionEls = document.querySelectorAll("[data-release-version]");
  const downloadBtns = document.querySelectorAll("[data-download-release]");

  try {
    const res = await fetch("https://api.github.com/repos/Sezergumus/beacon/releases", {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return;
    const releases = await res.json();
    if (!Array.isArray(releases) || releases.length === 0) return;

    const latest = releases[0];
    if (latest.tag_name) {
      versionEls.forEach((el) => {
        el.textContent = latest.tag_name;
      });
    }

    // Look for a Windows installer asset (.msi or .exe)
    const assets = Array.isArray(latest.assets) ? latest.assets : [];
    const winInstaller = assets.find(
      (a) =>
        typeof a.browser_download_url === "string" &&
        /\.(msi|exe)$/i.test(a.name || "")
    );

    const targetUrl =
      (winInstaller && winInstaller.browser_download_url) ||
      latest.html_url ||
      "https://github.com/Sezergumus/beacon/releases";

    downloadBtns.forEach((btn) => {
      btn.setAttribute("href", targetUrl);
    });
  } catch {
    // Offline or rate-limited; default links to https://github.com/Sezergumus/beacon/releases/latest remain active.
  }
}
