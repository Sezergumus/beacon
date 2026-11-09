import React, { useEffect, useState } from "react";
import { Check, Copy, Loader2, Puzzle, Skull, User } from "lucide-react";
import { formatAppError, ServerProperties, tauriApi, WorldDetails } from "../../lib/tauri";
import { useServerStore } from "../../stores/useServerStore";
import { Button, EmptyState, Modal, PlayerHead, Row, Segmented, TextInput, Toggle } from "../ui";

const CORE_LABELS = { paper: "Paper", fabric: "Fabric", neoforge: "NeoForge" } as const;

const formatSize = (bytes: number) =>
  bytes >= 1024 * 1024 * 1024
    ? `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
    : bytes >= 1024 * 1024
    ? `${(bytes / 1024 / 1024).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytes / 1024))} KB`;

const formatDuration = (secs: number) => {
  if (secs < 60) return secs === 0 ? "Not played yet" : "Under a minute";
  const mins = Math.floor(secs / 60);
  const hours = Math.floor(mins / 60);
  if (hours === 0) return `${mins} min`;
  return mins % 60 === 0 ? `${hours} h` : `${hours} h ${mins % 60} min`;
};

const formatDate = (ms: number) =>
  new Date(ms).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });

const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n));

const DIFFICULTIES = [
  { id: "peaceful", label: "Peaceful" },
  { id: "easy", label: "Easy" },
  { id: "normal", label: "Normal" },
  { id: "hard", label: "Hard" },
];

const GAMEMODES = [
  { id: "survival", label: "Survival" },
  { id: "creative", label: "Creative" },
  { id: "adventure", label: "Adventure" },
  { id: "spectator", label: "Spectator" },
];

const Stat: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
  <div className="rounded-2xl bg-raised p-4 min-w-0">
    <div className="text-xs font-bold uppercase tracking-wide text-faint">{label}</div>
    <div className="font-extrabold mt-1 truncate">{children}</div>
  </div>
);

const Section: React.FC<{ title: string; hint?: string; children: React.ReactNode }> = ({
  title,
  hint,
  children,
}) => (
  <section>
    <h3 className="text-lg font-bold">{title}</h3>
    {hint && <p className="text-sm text-soft mt-0.5">{hint}</p>}
    <div className="mt-3">{children}</div>
  </section>
);

const FileList: React.FC<{ files: WorldDetails["mods"] }> = ({ files }) => (
  <ul className="rounded-2xl bg-raised divide-y divide-line/60 max-h-64 overflow-y-auto">
    {files.map((f) => (
      <li key={f.filename} className="flex items-center justify-between gap-4 px-4 py-2.5 text-sm">
        <span className="font-bold truncate">{f.filename.replace(/\.jar$/i, "")}</span>
        <span className="text-soft shrink-0">{formatSize(f.size_bytes)}</span>
      </li>
    ))}
  </ul>
);

export const WorldDetailsModal: React.FC<{
  worldId: string;
  /** Put the cursor in the name box straight away (opened from "Rename"). */
  focusName?: boolean;
  onClose: () => void;
}> = ({ worldId, focusName = false, onClose }) => {
  const { status, worlds, pushToast, renameWorld, refreshWorlds } = useServerStore();
  const [details, setDetails] = useState<WorldDetails | null>(null);
  const [failed, setFailed] = useState(false);
  const [draft, setDraft] = useState<ServerProperties | null>(null);
  const [saved, setSaved] = useState<ServerProperties | null>(null);
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [renaming, setRenaming] = useState(false);

  const running = status !== "stopped" && !!worlds?.find((w) => w.id === worldId)?.active;

  useEffect(() => {
    let cancelled = false;
    setDetails(null);
    setFailed(false);
    tauriApi
      .getWorldDetails(worldId)
      .then((d) => {
        if (cancelled) return;
        setDetails(d);
        setDraft(d.properties);
        setSaved(d.properties);
        setName(d.name);
      })
      .catch((err) => {
        if (cancelled) return;
        setFailed(true);
        pushToast("error", "Couldn't open that world", formatAppError(err));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [worldId]);

  const edit = (patch: Partial<ServerProperties>) =>
    setDraft((d) => (d ? { ...d, ...patch } : d));

  const dirty = !!draft && !!saved && JSON.stringify(draft) !== JSON.stringify(saved);
  const nameChanged = !!details && name.trim() !== "" && name.trim() !== details.name;

  const saveSettings = async () => {
    if (!draft) return;
    const clean: ServerProperties = {
      ...draft,
      motd: draft.motd.replace(/[\r\n]+/g, " "),
      max_players: clamp(Math.round(draft.max_players) || 1, 1, 1000),
      view_distance: clamp(Math.round(draft.view_distance) || 10, 2, 32),
    };
    setSaving(true);
    try {
      const next = await tauriApi.saveWorldProperties(worldId, clean);
      setDraft(next);
      setSaved(next);
      pushToast("success", "Saved", "This world's settings were updated.");
    } catch (err) {
      pushToast("error", "Couldn't save the settings", formatAppError(err));
    } finally {
      setSaving(false);
    }
  };

  const saveName = async () => {
    if (!details || !nameChanged) return;
    setRenaming(true);
    if (await renameWorld(worldId, name)) {
      setDetails({ ...details, name: name.trim() });
      setName(name.trim());
    }
    setRenaming(false);
  };

  const copySeed = async () => {
    if (!details?.seed) return;
    try {
      await navigator.clipboard.writeText(details.seed);
      pushToast("success", "Copied", "The seed is on your clipboard.");
    } catch {
      pushToast("error", "Couldn't copy", "Select the seed and copy it by hand.");
    }
  };

  const close = () => {
    refreshWorlds();
    onClose();
  };

  const subtitle = details
    ? [
        details.core && details.mc_version
          ? `${CORE_LABELS[details.core]} ${details.mc_version}`
          : details.mc_version,
        details.last_played_ms ? `last played ${formatDate(details.last_played_ms)}` : "never played",
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined;

  const isPaper = details?.core === "paper";
  const mainFiles = details ? (isPaper ? details.plugins : details.mods) : [];
  const otherFiles = details ? (isPaper ? details.mods : details.plugins) : [];
  const mainLabel = isPaper ? "Plugins" : "Mods";
  const otherLabel = isPaper ? "Mods" : "Plugins";

  return (
    <Modal title={details?.name ?? "World"} subtitle={subtitle} onClose={close}>
      {!details || !draft ? (
        <div className="py-16 flex justify-center text-soft">
          {failed ? (
            <Button onClick={close}>Close</Button>
          ) : (
            <Loader2 className="w-6 h-6 animate-spin" />
          )}
        </div>
      ) : (
        <>
          {running && (
            <div className="rounded-2xl bg-brand/15 p-3.5 text-sm font-bold">
              This world is open right now. Numbers come from its last save, and difficulty, keep
              inventory and whitelist changes apply straight away.
            </div>
          )}

          <form
            className="flex items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              saveName();
            }}
          >
            <label htmlFor="world-name" className="text-sm font-bold text-soft shrink-0">
              World name
            </label>
            <TextInput
              id="world-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={40}
              autoFocus={focusName}
              onFocus={(e) => focusName && e.currentTarget.select()}
              className="h-10"
            />
            <Button
              type="submit"
              variant="primary"
              disabled={!nameChanged || renaming}
              icon={renaming ? <Loader2 className="w-4 h-4 animate-spin" /> : <Check className="w-4 h-4" />}
            >
              Rename
            </Button>
          </form>

          <Section title="Overview">
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
              <Stat label="Time played">{formatDuration(details.total_playtime_secs)}</Stat>
              <Stat label="Size on disk">{formatSize(details.size_bytes)}</Stat>
              <Stat label="Created">{new Date(details.created_ms).toLocaleDateString()}</Stat>
              <Stat label="Game version">
                {details.game_version ?? details.mc_version ?? "—"}
              </Stat>
              <Stat label="In-game age">
                {details.world_age_ticks === null
                  ? "—"
                  : `${Math.floor(details.world_age_ticks / 24000)} days`}
              </Stat>
              <Stat label="Spawn">
                {details.spawn ? details.spawn.join(", ") : "—"}
              </Stat>
            </div>
            <div
              className={`grid grid-cols-1 gap-3 mt-3 ${
                details.hardcore ? "sm:grid-cols-[1fr_auto]" : ""
              }`}
            >
              <div className="rounded-2xl bg-raised p-4 min-w-0 flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="text-xs font-bold uppercase tracking-wide text-faint">Seed</div>
                  <div className="font-extrabold mt-1 truncate select-text">
                    {details.seed ?? "Shown after the world is first started"}
                  </div>
                </div>
                {details.seed && (
                  <Button size="sm" variant="ghost" onClick={copySeed} icon={<Copy className="w-4 h-4" />}>
                    Copy
                  </Button>
                )}
              </div>
              {details.hardcore && (
                <div className="rounded-2xl bg-danger/15 text-danger px-4 flex items-center gap-2 font-extrabold">
                  <Skull className="w-5 h-5" /> Hardcore
                </div>
              )}
            </div>
          </Section>

          <Section
            title={`Players (${details.players.length})`}
            hint="Time each person has spent in this world."
          >
            {details.players.length === 0 ? (
              <div className="rounded-2xl bg-raised">
                <EmptyState
                  icon={User}
                  title="Nobody has played here yet"
                  text="Play time shows up once someone has joined and the world has saved."
                />
              </div>
            ) : (
              <ul className="rounded-2xl bg-raised divide-y divide-line/60 max-h-72 overflow-y-auto">
                {details.players.map((p) => (
                  <li key={p.uuid} className="flex items-center gap-3 px-4 py-3">
                    <PlayerHead name={p.name ?? "?"} size={36} />
                    <div className="flex-1 min-w-0">
                      <div className="font-bold truncate">
                        {p.name ?? `Unknown player (${p.uuid.slice(0, 8)})`}
                      </div>
                      {p.last_seen_ms && (
                        <div className="text-sm text-soft">last seen {formatDate(p.last_seen_ms)}</div>
                      )}
                    </div>
                    <div className="font-extrabold shrink-0">{formatDuration(p.playtime_secs)}</div>
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section
            title={`${mainLabel} (${mainFiles.length})`}
            hint={
              worlds?.find((w) => w.id === worldId)?.active
                ? `Add or remove ${mainLabel.toLowerCase()} on the Mods page.`
                : `Select this world with “Use this”, then manage ${mainLabel.toLowerCase()} on the Mods page.`
            }
          >
            {mainFiles.length === 0 ? (
              <div className="rounded-2xl bg-raised">
                <EmptyState
                  icon={Puzzle}
                  title={`No ${mainLabel.toLowerCase()} installed`}
                  text="This world runs without any."
                />
              </div>
            ) : (
              <FileList files={mainFiles} />
            )}
            {otherFiles.length > 0 && (
              <div className="mt-4">
                <div className="font-bold mb-2">
                  {otherLabel} ({otherFiles.length})
                </div>
                <FileList files={otherFiles} />
              </div>
            )}
          </Section>

          <Section title="Settings" hint="Saved to this world only.">
            <div className="rounded-2xl bg-raised px-5 py-4">
              <Row title="Game mode" hint="What new players start in.">
                <Segmented
                  value={draft.gamemode}
                  options={GAMEMODES}
                  onChange={(gamemode) => edit({ gamemode })}
                />
              </Row>
              <Row title="Difficulty">
                <Segmented
                  value={draft.difficulty}
                  options={DIFFICULTIES}
                  onChange={(difficulty) => edit({ difficulty })}
                />
              </Row>
              <Row title="Keep inventory" hint="Players keep their items when they die.">
                <Toggle
                  label="Keep inventory"
                  checked={draft.keep_inventory}
                  onChange={(keep_inventory) => edit({ keep_inventory })}
                />
              </Row>
              <Row title="Players can fight each other" hint="PvP">
                <Toggle label="PvP" checked={draft.pvp} onChange={(pvp) => edit({ pvp })} />
              </Row>
              <Row title="Only invited friends" hint="Whitelist: only people you add can join.">
                <Toggle
                  label="Whitelist"
                  checked={draft.white_list}
                  onChange={(white_list) => edit({ white_list })}
                />
              </Row>
              <Row title="Welcome message" hint="Shown in the server list.">
                <TextInput
                  value={draft.motd}
                  onChange={(e) => edit({ motd: e.target.value })}
                  maxLength={120}
                  aria-label="Welcome message"
                  className="h-9 w-72 bg-surface"
                />
              </Row>
              <Row title="Most players at once">
                <TextInput
                  type="number"
                  min={1}
                  max={1000}
                  value={draft.max_players}
                  onChange={(e) => edit({ max_players: Number(e.target.value) })}
                  aria-label="Most players at once"
                  className="h-9 w-24 bg-surface"
                />
              </Row>
              <Row title="View distance" hint="In chunks. Lower is easier on your PC (2–32).">
                <TextInput
                  type="number"
                  min={2}
                  max={32}
                  value={draft.view_distance}
                  onChange={(e) => edit({ view_distance: Number(e.target.value) })}
                  aria-label="View distance"
                  className="h-9 w-24 bg-surface"
                />
              </Row>
            </div>
            <div className="flex items-center justify-end gap-2 mt-4">
              {dirty && (
                <Button variant="ghost" onClick={() => setDraft(saved)} disabled={saving}>
                  Undo changes
                </Button>
              )}
              <Button
                variant="primary"
                onClick={saveSettings}
                disabled={!dirty || saving}
                icon={saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Check className="w-4 h-4" />}
              >
                Save settings
              </Button>
            </div>
          </Section>
        </>
      )}
    </Modal>
  );
};
