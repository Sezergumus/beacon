import React, { useEffect, useState } from "react";
import {
  Archive,
  FolderOpen,
  ArrowRightLeft,
  Globe,
  Handshake,
  HardDrive,
  Loader2,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import { useNetworkStore } from "../../stores/useNetworkStore";
import { useServerStore } from "../../stores/useServerStore";
import { formatAppError, tauriApi, WorldEntry } from "../../lib/tauri";
import {
  Button,
  Card,
  Disclosure,
  EmptyState,
  Page,
  PlayerHead,
  Toggle,
} from "../ui";
import { NewWorldForm } from "./NewWorldForm";
import { WorldDetailsModal } from "./WorldDetailsModal";

const timeAgo = (ms: number) => {
  const mins = Math.round((Date.now() - ms) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  return hours < 24 ? `${hours} h ago` : new Date(ms).toLocaleDateString();
};

const CORE_LABELS = {
  paper: "Paper",
  fabric: "Fabric",
  neoforge: "NeoForge",
} as const;

const formatSize = (bytes: number) =>
  bytes >= 1024 * 1024 * 1024
    ? `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
    : bytes >= 1024 * 1024
      ? `${(bytes / 1024 / 1024).toFixed(1)} MB`
      : `${Math.max(1, Math.round(bytes / 1024))} KB`;

export const WorldsView: React.FC = () => {
  const {
    network,
    isBusy,
    migrationBanner,
    toggleTrustedCohost,
    createWorldSnapshotNow,
    transferHostToPeer,
    refreshNetworkState,
  } = useNetworkStore();
  const {
    status,
    pushToast,
    worlds,
    selectedWorldId,
    selectWorld,
    refreshWorlds,
  } = useServerStore();

  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [details, setDetails] = useState<{
    id: string;
    rename: boolean;
  } | null>(null);

  const peers = network.connected_peers;
  const serverStopped = status === "stopped";
  const latestSnapshotId = network.latest_snapshot?.snapshot_id;

  // Reload when the page opens, a backup is made, or the server starts/stops.
  useEffect(() => {
    refreshWorlds();
  }, [refreshWorlds, latestSnapshotId, status]);

  const remove = async (world: WorldEntry) => {
    setDeletingId(world.id);
    try {
      await tauriApi.deleteWorld(world.id);
      pushToast(
        "info",
        "Deleted",
        world.kind === "world"
          ? `“${world.name}” was deleted.`
          : "That backup was deleted.",
      );
      setConfirmId(null);
      await Promise.all([refreshWorlds(), refreshNetworkState()]);
    } catch (err) {
      pushToast("error", "Couldn't delete that", formatAppError(err));
    } finally {
      setDeletingId(null);
    }
  };

  return (
    <Page
      title="Worlds"
      subtitle="Your worlds, backed up every 10 minutes."
      action={
        <div className="flex items-center gap-2">
          <Button
            onClick={() =>
              tauriApi
                .openDataFolder()
                .catch((err) => pushToast("error", "Couldn't open the folder", formatAppError(err)))
            }
            icon={<FolderOpen className="w-4 h-4" />}
            title="Where worlds, settings and server icons are saved"
          >
            Open folder
          </Button>
          <Button
            onClick={() => setCreating((v) => !v)}
            icon={<Plus className="w-4 h-4" />}
          >
            New world
          </Button>
          <Button
            variant="primary"
            onClick={createWorldSnapshotNow}
            disabled={isBusy}
            icon={
              isBusy ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Archive className="w-4 h-4" />
              )
            }
          >
            Back up now
          </Button>
        </div>
      }
    >
      {migrationBanner && migrationBanner.stage !== "completed" && (
        <div className="rounded-2xl bg-brand/15 p-4 flex items-center gap-3">
          <Loader2 className="w-5 h-5 text-brand animate-spin shrink-0" />
          <div className="font-bold">{migrationBanner.message}</div>
        </div>
      )}

      {creating && (
        <div className="rounded-2xl bg-surface p-4">
          <NewWorldForm
            placeholder="Name your new world, e.g. Survival with friends"
            autoFocus
            onCancel={() => setCreating(false)}
            onCreated={() => setCreating(false)}
          />
        </div>
      )}

      <Card>
        {worlds === null ? (
          <div className="py-8 flex justify-center text-soft">
            <Loader2 className="w-5 h-5 animate-spin" />
          </div>
        ) : worlds.length === 0 ? (
          <EmptyState
            icon={Globe}
            title="No worlds yet"
            text="Tap “New world” to name your first one, then start it from Home."
          />
        ) : (
          <ul className="flex flex-col gap-1 -mx-3">
            {worlds.map((w) => {
              const isLive = w.kind === "world";
              const confirming = confirmId === w.id;
              const deleting = deletingId === w.id;
              const inUse = isLive && w.active && !serverStopped;
              const blocked = inUse;
              const selected =
                isLive && serverStopped && w.id === selectedWorldId;
              return (
                <li
                  key={w.id}
                  className={`flex items-center gap-4 px-3 py-3 rounded-xl ${
                    isLive ? "cursor-pointer hover:bg-raised/60" : ""
                  }`}
                  {...(isLive && {
                    role: "button",
                    tabIndex: 0,
                    "aria-label": `Open details of ${w.name}`,
                    onClick: (e: React.MouseEvent) => {
                      // Buttons and the rename box inside the row do their own thing.
                      if (
                        (e.target as HTMLElement).closest("button, input, form")
                      )
                        return;
                      setDetails({ id: w.id, rename: false });
                    },
                    onKeyDown: (e: React.KeyboardEvent) => {
                      if (
                        e.target === e.currentTarget &&
                        (e.key === "Enter" || e.key === " ")
                      ) {
                        e.preventDefault();
                        setDetails({ id: w.id, rename: false });
                      }
                    },
                  })}
                >
                  <div className="w-11 h-11 rounded-xl bg-raised flex items-center justify-center text-soft shrink-0">
                    {isLive ? (
                      <Globe className="w-5 h-5 text-brand" />
                    ) : (
                      <HardDrive className="w-5 h-5" />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-bold truncate flex items-center gap-2">
                      {isLive
                        ? w.name
                        : `Backup · ${new Date(w.modified_ms).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}`}
                      {inUse && (
                        <span className="text-xs font-extrabold px-2 py-0.5 rounded-full bg-brand/15 text-brand">
                          In use
                        </span>
                      )}
                      {selected && (
                        <span className="text-xs font-extrabold px-2 py-0.5 rounded-full bg-brand/15 text-brand">
                          Opens next
                        </span>
                      )}
                    </div>
                    <div className="text-sm text-soft">
                      {isLive && w.core && w.mc_version && (
                        <>
                          <span className="font-bold text-ink">
                            {CORE_LABELS[w.core]} {w.mc_version}
                          </span>
                          {" · "}
                          {w.core === "paper"
                            ? `${w.plugin_count} ${w.plugin_count === 1 ? "plugin" : "plugins"}`
                            : `${w.mod_count} ${w.mod_count === 1 ? "mod" : "mods"}`}
                          {" · "}
                        </>
                      )}
                      {formatSize(w.size_bytes)} ·{" "}
                      {isLive ? "last played" : "saved"}{" "}
                      {timeAgo(w.modified_ms)}
                    </div>
                  </div>

                  {isLive && !confirming && (
                    <>
                      {serverStopped && !selected && (
                        <Button size="sm" onClick={() => selectWorld(w.id)}>
                          Use this
                        </Button>
                      )}
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setDetails({ id: w.id, rename: true })}
                        title="Rename"
                        icon={<Pencil className="w-4 h-4" />}
                      >
                        Edit
                      </Button>
                    </>
                  )}

                  {confirming ? (
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-bold text-danger mr-1">
                        Delete for good?
                      </span>
                      <Button
                        size="sm"
                        onClick={() => setConfirmId(null)}
                        disabled={deleting}
                      >
                        Keep
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => remove(w)}
                        disabled={deleting}
                        icon={
                          deleting ? (
                            <Loader2 className="w-4 h-4 animate-spin" />
                          ) : (
                            <Trash2 className="w-4 h-4" />
                          )
                        }
                      >
                        Delete
                      </Button>
                    </div>
                  ) : (
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => setConfirmId(w.id)}
                      disabled={blocked}
                      title={blocked ? "Close your world first" : "Delete"}
                      icon={<Trash2 className="w-4 h-4" />}
                    >
                      Delete
                    </Button>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </Card>

      <Card
        title="Friends who keep a copy"
        hint="Turn this on for friends you trust. If your PC goes offline, one of them can keep the world going."
      >
        {peers.length === 0 ? (
          <EmptyState
            icon={Handshake}
            title="No friends connected"
            text={
              network.role === "host"
                ? "Once a friend joins with your code, they'll show up here."
                : "Share your world from Home first, then friends will show up here."
            }
          />
        ) : (
          <ul className="flex flex-col">
            {peers.map((peer) => (
              <li
                key={peer.node_id}
                className="flex items-center gap-4 py-3 border-b border-line/60 last:border-0"
              >
                <PlayerHead name={peer.display_name} size={40} />
                <div className="flex-1 min-w-0">
                  <div className="font-bold truncate">{peer.display_name}</div>
                  <div className="text-sm text-soft">
                    {peer.has_latest_snapshot
                      ? "Has the latest backup"
                      : "No backup yet"}{" "}
                    · {peer.ping_ms} ms
                  </div>
                </div>
                <label className="flex items-center gap-2.5 text-sm font-bold text-soft">
                  Keeps a copy
                  <Toggle
                    checked={peer.is_trusted_cohost}
                    onChange={(v) => toggleTrustedCohost(peer.node_id, v)}
                    label={`Let ${peer.display_name} keep a copy`}
                  />
                </label>
                {network.role === "host" && (
                  <Button
                    size="sm"
                    onClick={() => transferHostToPeer(peer.node_id)}
                    disabled={isBusy}
                    icon={<ArrowRightLeft className="w-4 h-4" />}
                  >
                    Let them host
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
      </Card>

      <div className="px-2 pb-4">
        <Disclosure label="Connection details">
          <dl className="rounded-2xl bg-surface p-5 grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
            <dt className="text-soft">Your ID</dt>
            <dd className="font-mono truncate select-text">
              {network.node_id}
            </dd>
            <dt className="text-soft">Relay</dt>
            <dd className="font-mono truncate">
              {network.relay_url ?? "Public relay"}
            </dd>
            <dt className="text-soft">Connection</dt>
            <dd className="font-mono truncate">
              {network.upnp_external_addr ?? "Direct (hole punching)"}
            </dd>
            {network.local_lan_port && (
              <>
                <dt className="text-soft">LAN port</dt>
                <dd className="font-mono">{network.local_lan_port}</dd>
              </>
            )}
          </dl>
        </Disclosure>
      </div>

      {details && (
        <WorldDetailsModal
          worldId={details.id}
          focusName={details.rename}
          onClose={() => setDetails(null)}
        />
      )}
    </Page>
  );
};
