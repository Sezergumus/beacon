import React, { useEffect, useState } from "react";
import {
  AlertTriangle,
  BookOpen,
  Bug,
  CheckCircle2,
  Code2,
  Download,
  ExternalLink,
  Heart,
  Loader2,
  Puzzle,
  Scale,
  X,
} from "lucide-react";
import {
  formatAppError,
  ModrinthProjectDetails,
  ModrinthSearchHit,
  openExternal,
  ServerCore,
  tauriApi,
} from "../../lib/tauri";
import { formatCount } from "../../lib/format";
import { useServerStore } from "../../stores/useServerStore";
import { Button, Disclosure, InstalledBadge, Modal } from "../ui";
import { DiscordIcon } from "../icons";
import { Markdown } from "./Markdown";

const CORE_LABELS: Record<ServerCore, string> = { paper: "Paper", fabric: "Fabric", neoforge: "NeoForge" };

/** Loader names Modrinth uses for what each server type can run. */
const CORE_LOADERS: Record<ServerCore, string[]> = {
  paper: ["paper", "spigot", "bukkit", "purpur"],
  fabric: ["fabric"],
  neoforge: ["neoforge"],
};

const TYPE_LABELS: Record<string, string> = { mod: "Mod", plugin: "Plugin", modpack: "Modpack" };

const formatDate = (iso: string) => {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "—" : d.toLocaleDateString([], { dateStyle: "medium" });
};

const prettyTag = (tag: string) => tag.replace(/-/g, " ");

const Chip: React.FC<{ children: React.ReactNode; title?: string }> = ({ children, title }) => (
  <span
    title={title}
    className="inline-flex items-center gap-1.5 h-7 px-2.5 rounded-lg bg-raised text-sm font-bold text-soft capitalize"
  >
    {children}
  </span>
);

const Notice: React.FC<{ tone: "ok" | "warn"; icon: React.ReactNode; children: React.ReactNode }> = ({
  tone,
  icon,
  children,
}) => (
  <div
    className={`rounded-2xl p-3.5 text-sm font-bold flex items-start gap-2.5 ${
      tone === "ok" ? "bg-brand/15" : "bg-sun/15"
    }`}
  >
    <span className={`mt-0.5 shrink-0 ${tone === "ok" ? "text-brand" : "text-sun"}`}>{icon}</span>
    <div>{children}</div>
  </div>
);

/** Does this project have a build for the world's server type and Minecraft version? */
const CompatNotice: React.FC<{
  details: ModrinthProjectDetails;
  core: ServerCore;
  mcVersion: string;
}> = ({ details, core, mcVersion }) => {
  const loaderOk = details.loaders.some((l) => CORE_LOADERS[core].includes(l));
  const versionOk = details.game_versions.includes(mcVersion);
  const label = `${CORE_LABELS[core]} ${mcVersion}`;

  if (loaderOk && versionOk) {
    return (
      <Notice tone="ok" icon={<CheckCircle2 className="w-5 h-5" />}>
        Works with this world ({label}).
      </Notice>
    );
  }
  const newest = details.game_versions[details.game_versions.length - 1];
  return (
    <Notice tone="warn" icon={<AlertTriangle className="w-5 h-5" />}>
      {!loaderOk
        ? `This doesn't list support for ${CORE_LABELS[core]}.`
        : `No build for ${mcVersion} yet${newest ? ` — the newest it supports is ${newest}` : ""}.`}{" "}
      Adding it may fail or not work in this world ({label}).
    </Notice>
  );
};

const Lightbox: React.FC<{ src: string; caption?: string | null; onClose: () => void }> = ({
  src,
  caption,
  onClose,
}) => {
  useEffect(() => {
    // Esc closes the picture only, not the whole window behind it.
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 bg-black/85 flex flex-col items-center justify-center gap-3 p-8 cursor-zoom-out"
      onClick={onClose}
    >
      <button
        aria-label="Close picture"
        className="absolute top-5 right-5 p-2 rounded-xl text-white/80 hover:text-white hover:bg-white/10 cursor-pointer"
      >
        <X className="w-6 h-6" />
      </button>
      <img src={src} alt={caption ?? ""} className="max-w-full max-h-[80vh] rounded-xl object-contain" />
      {caption && <p className="text-white/90 font-bold">{caption}</p>}
    </div>
  );
};

const LinkButton: React.FC<{ url?: string | null; icon: React.ReactNode; children: React.ReactNode }> = ({
  url,
  icon,
  children,
}) =>
  url ? (
    <Button size="sm" onClick={() => openExternal(url).catch(() => {})} icon={icon}>
      {children}
    </Button>
  ) : null;

export const ModDetailsModal: React.FC<{ hit: ModrinthSearchHit; onClose: () => void }> = ({ hit, onClose }) => {
  const { selectedCore, selectedVersion, installingSlug, installedProjectIds, installModrinthItem, removeProject, removingProjectId, pushToast } =
    useServerStore();
  const [details, setDetails] = useState<ModrinthProjectDetails | null>(null);
  const [failed, setFailed] = useState(false);
  const [zoomed, setZoomed] = useState<{ src: string; caption?: string | null } | null>(null);
  const [fullBody, setFullBody] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setDetails(null);
    setFailed(false);
    tauriApi
      .getModrinthProject(hit.project_id)
      .then((d) => !cancelled && setDetails(d))
      .catch((err) => {
        if (cancelled) return;
        setFailed(true);
        pushToast("error", "Couldn't load the details", formatAppError(err));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hit.project_id]);

  const type = details?.project_type ?? hit.project_type;
  const installing = installingSlug === hit.slug;
  // Modpacks are unpacked into many files, so there is no single file to recognise.
  const removing = removingProjectId === hit.project_id;
  const installed = hit.project_type !== "modpack" && (removing || installedProjectIds.includes(hit.project_id));
  const downloads = details?.downloads ?? hit.downloads;
  const categories = details?.categories ?? hit.categories;
  const icon = details?.icon_url ?? hit.icon_url;
  const pageUrl = details?.page_url ?? `https://modrinth.com/${hit.project_type}/${hit.slug}`;
  const longBody = (details?.body.length ?? 0) > 1200;
  const isAddon = type === "mod" || type === "plugin";

  return (
    <>
      <Modal
        title={details?.title ?? hit.title}
        subtitle={`${TYPE_LABELS[type] ?? "Project"} by ${hit.author}`}
        onClose={onClose}
      >
        {/* Summary */}
        <div className="flex items-start gap-4">
          {icon ? (
            <img src={icon} alt="" className="w-20 h-20 rounded-2xl object-cover bg-raised shrink-0" />
          ) : (
            <div className="w-20 h-20 rounded-2xl bg-raised flex items-center justify-center text-soft shrink-0">
              <Puzzle className="w-8 h-8" />
            </div>
          )}
          <div className="flex-1 min-w-0">
            <p className="text-base select-text">{details?.description ?? hit.description}</p>
            <div className="flex flex-wrap gap-2 mt-3">
              <Chip title="Downloads">
                <Download className="w-3.5 h-3.5" /> {formatCount(downloads)}
              </Chip>
              {details && (
                <Chip title="Followers">
                  <Heart className="w-3.5 h-3.5" /> {formatCount(details.followers)}
                </Chip>
              )}
              {details && <Chip title="Last updated">Updated {formatDate(details.updated)}</Chip>}
              {details?.license && (
                <Chip title={`License: ${details.license}`}>
                  <Scale className="w-3.5 h-3.5 shrink-0" />
                  <span className="truncate max-w-[14rem] normal-case">{details.license}</span>
                </Chip>
              )}
              {categories.map((c) => (
                <Chip key={c}>{prettyTag(c)}</Chip>
              ))}
            </div>
          </div>
          {installed ? (
            <InstalledBadge onUninstall={() => removeProject(hit.project_id)} removing={removing} />
          ) : (
            <Button
              variant="primary"
              onClick={() => installModrinthItem(hit)}
              disabled={installing || installingSlug !== null}
              icon={installing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Download className="w-4 h-4" />}
            >
              {installing ? "Adding…" : "Add"}
            </Button>
          )}
        </div>

        {!details ? (
          <div className="py-10 flex flex-col items-center gap-4 text-soft">
            {failed ? (
              <>
                <p>The full page couldn't be loaded.</p>
                <Button onClick={() => openExternal(pageUrl).catch(() => {})} icon={<ExternalLink className="w-4 h-4" />}>
                  Open on Modrinth
                </Button>
              </>
            ) : (
              <Loader2 className="w-6 h-6 animate-spin" />
            )}
          </div>
        ) : (
          <>
            <div className="flex flex-col gap-3">
              <CompatNotice details={details} core={selectedCore} mcVersion={selectedVersion} />
              {isAddon && details.server_side === "unsupported" && (
                <Notice tone="warn" icon={<AlertTriangle className="w-5 h-5" />}>
                  This is meant for players' own game only. It won't do anything on the server.
                </Notice>
              )}
              {isAddon && details.client_side === "required" && details.server_side !== "unsupported" && (
                <Notice tone="ok" icon={<CheckCircle2 className="w-5 h-5" />}>
                  Friends need it too. Beacon gives it to them automatically when they join.
                </Notice>
              )}
            </div>

            {details.gallery.length > 0 && (
              <section>
                <h3 className="text-lg font-bold">Pictures</h3>
                <div className="mt-3 -mx-6 px-6 flex gap-3 overflow-x-auto snap-x pb-2">
                  {details.gallery.map((g) => (
                    <button
                      key={g.url}
                      onClick={() => setZoomed({ src: g.url, caption: g.title })}
                      title={g.title ?? undefined}
                      className="snap-start shrink-0 rounded-xl overflow-hidden bg-raised cursor-zoom-in focus:outline-none focus:ring-2 focus:ring-brand/60"
                    >
                      <img src={g.url} alt={g.title ?? ""} loading="lazy" className="h-40 w-auto object-cover" />
                    </button>
                  ))}
                </div>
              </section>
            )}

            {details.body.trim() && (
              <section>
                <h3 className="text-lg font-bold">About</h3>
                <div
                  className={`mt-3 relative ${longBody && !fullBody ? "max-h-96 overflow-hidden" : ""}`}
                >
                  <Markdown>{details.body}</Markdown>
                  {longBody && !fullBody && (
                    <div className="absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-surface to-transparent" />
                  )}
                </div>
                {longBody && (
                  <Button size="sm" variant="ghost" className="mt-2" onClick={() => setFullBody((v) => !v)}>
                    {fullBody ? "Show less" : "Show the full description"}
                  </Button>
                )}
              </section>
            )}

            {details.versions.length > 0 && (
              <Disclosure label={`Recent versions (${details.versions.length})`}>
                <ul className="rounded-2xl bg-raised divide-y divide-line/60 max-h-72 overflow-y-auto">
                  {details.versions.map((v) => (
                    <li key={`${v.version_number}-${v.date_published}`} className="px-4 py-2.5 text-sm">
                      <div className="flex items-center gap-2">
                        <span className="font-bold truncate">{v.name || v.version_number}</span>
                        {v.version_type !== "release" && (
                          <span className="text-xs font-extrabold uppercase text-sun shrink-0">{v.version_type}</span>
                        )}
                        <span className="ml-auto text-soft shrink-0">{formatDate(v.date_published)}</span>
                      </div>
                      <div className="text-soft mt-0.5 truncate">
                        {v.game_versions.slice(-4).reverse().join(", ")}
                        {v.game_versions.length > 4 ? "…" : ""} · {v.loaders.join(", ")}
                      </div>
                    </li>
                  ))}
                </ul>
              </Disclosure>
            )}

            <div className="flex flex-wrap gap-2">
              <LinkButton url={pageUrl} icon={<ExternalLink className="w-4 h-4" />}>
                Modrinth page
              </LinkButton>
              <LinkButton url={details.source_url} icon={<Code2 className="w-4 h-4" />}>
                Source code
              </LinkButton>
              <LinkButton url={details.issues_url} icon={<Bug className="w-4 h-4" />}>
                Report a problem
              </LinkButton>
              <LinkButton url={details.wiki_url} icon={<BookOpen className="w-4 h-4" />}>
                Wiki
              </LinkButton>
              <LinkButton url={details.discord_url} icon={<DiscordIcon className="w-4 h-4" />}>
                Discord
              </LinkButton>
            </div>
          </>
        )}
      </Modal>
      {zoomed && <Lightbox src={zoomed.src} caption={zoomed.caption} onClose={() => setZoomed(null)} />}
    </>
  );
};
