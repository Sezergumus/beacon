import React, { useEffect, useState } from "react";
import { Loader2, Plus, ShieldCheck, X } from "lucide-react";
import { formatAppError, tauriApi, WhitelistEntry } from "../../lib/tauri";
import { useServerStore } from "../../stores/useServerStore";
import { Button, EmptyState, PlayerHead, TextInput } from "../ui";

/** Who may join a world when "Only invited friends" is on. */
export const WhitelistSection: React.FC<{
  worldId: string;
  /** Whether the whitelist switch is on (saved or not). */
  enabled: boolean;
  /** Names of people who already played here, offered as one-click adds. */
  knownPlayers: string[];
}> = ({ worldId, enabled, knownPlayers }) => {
  const pushToast = useServerStore((s) => s.pushToast);
  const [list, setList] = useState<WhitelistEntry[] | null>(null);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setList(null);
    tauriApi
      .getWhitelist(worldId)
      .then((l) => !cancelled && setList(l))
      .catch((err) => {
        if (cancelled) return;
        setList([]);
        pushToast("error", "Couldn't load the invite list", formatAppError(err));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [worldId]);

  const add = async (raw: string) => {
    const value = raw.trim();
    if (!value || busy) return;
    setBusy(value);
    try {
      setList(await tauriApi.addWhitelistPlayer(worldId, value));
      setName("");
    } catch (err) {
      pushToast("error", "Couldn't add that player", formatAppError(err));
    } finally {
      setBusy(null);
    }
  };

  const remove = async (player: string) => {
    if (busy) return;
    setBusy(player);
    try {
      setList(await tauriApi.removeWhitelistPlayer(worldId, player));
    } catch (err) {
      pushToast("error", "Couldn't remove that player", formatAppError(err));
    } finally {
      setBusy(null);
    }
  };

  const suggestions = list
    ? knownPlayers.filter((p) => !list.some((e) => e.name.toLowerCase() === p.toLowerCase()))
    : [];

  return (
    <div>
      {!enabled && (
        <div className="rounded-2xl bg-raised p-3.5 text-sm text-soft mb-3">
          “Only invited friends” is off, so anyone with the room code can join. This list is
          kept for when you turn it on.
        </div>
      )}

      <form
        className="flex items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          add(name);
        }}
      >
        <TextInput
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Friend's Minecraft name"
          maxLength={16}
          aria-label="Friend's Minecraft name"
          className="h-10"
        />
        <Button
          type="submit"
          variant="primary"
          disabled={!name.trim() || !!busy}
          icon={
            busy === name.trim() ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : (
              <Plus className="w-4 h-4" />
            )
          }
        >
          Invite
        </Button>
      </form>

      {suggestions.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 mt-3">
          <span className="text-sm text-soft">Played here:</span>
          {suggestions.map((p) => (
            <Button key={p} size="sm" variant="ghost" disabled={!!busy} onClick={() => add(p)}>
              + {p}
            </Button>
          ))}
        </div>
      )}

      <div className="mt-3">
        {list === null ? (
          <div className="py-8 flex justify-center text-soft">
            <Loader2 className="w-5 h-5 animate-spin" />
          </div>
        ) : list.length === 0 ? (
          <div className="rounded-2xl bg-raised">
            <EmptyState
              icon={ShieldCheck}
              title="Nobody is invited yet"
              text="Type a friend's name above. With “Only invited friends” on, only these people can join."
            />
          </div>
        ) : (
          <ul className="rounded-2xl bg-raised divide-y divide-line/60 max-h-64 overflow-y-auto">
            {list.map((e) => (
              <li key={e.uuid} className="flex items-center gap-3 px-4 py-2.5">
                <PlayerHead name={e.name} size={32} />
                <span className="flex-1 min-w-0 font-bold truncate">{e.name}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={!!busy}
                  onClick={() => remove(e.name)}
                  aria-label={`Remove ${e.name}`}
                  icon={
                    busy === e.name ? (
                      <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                      <X className="w-4 h-4" />
                    )
                  }
                >
                  Remove
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
};
