/* Beacon website: gentle scroll reveal + latest release link */

document.documentElement.classList.add("js");

document.addEventListener("DOMContentLoaded", () => {
  initReveal();
  fetchLatestRelease();
});

function initReveal() {
  const items = document.querySelectorAll(".reveal");
  if (!("IntersectionObserver" in window)) {
    items.forEach((el) => el.classList.add("is-visible"));
    return;
  }
  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.15 }
  );
  items.forEach((el) => observer.observe(el));
}

async function fetchLatestRelease() {
  try {
    const res = await fetch("https://api.github.com/repos/Sezergumus/beacon/releases", {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return;
    const releases = await res.json();
    if (!Array.isArray(releases) || releases.length === 0) return;

    const latest = releases[0];
    if (latest.tag_name) {
      document.querySelectorAll("[data-release-version]").forEach((el) => {
        el.textContent = latest.tag_name;
      });
    }

    // Point download buttons straight at the Windows installer when there is one
    const assets = Array.isArray(latest.assets) ? latest.assets : [];
    const installer = assets.find((a) => /\.(msi|exe)$/i.test(a.name || ""));
    const url = (installer && installer.browser_download_url) || latest.html_url;
    if (!url) return;
    document.querySelectorAll("[data-download-release]").forEach((btn) => {
      btn.setAttribute("href", url);
    });
  } catch {
    // Offline or rate-limited: the default /releases/latest links still work.
  }
}
