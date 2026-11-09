import React, { useEffect, useState } from "react";
import {
  Compass,
  Crown,
  Drumstick,
  Gift,
  Heart,
  MoreHorizontal,
  Package,
  Search,
  Sparkles,
  Users,
  X,
} from "lucide-react";
import { useServerStore } from "../../stores/useServerStore";
import { InventoryItemSlot, OnlinePlayer } from "../../lib/tauri";
import { POPULAR_MINECRAFT_ITEMS, resolveItemVisual } from "../../lib/minecraft-items";
import {
  Button,
  Card,
  Disclosure,
  EmptyState,
  Page,
  PlayerHead,
  Segmented,
  TextInput,
} from "../ui";

export const PlayersView: React.FC = () => {
  const { telemetry, status, activeInventory, openPlayerInventory } = useServerStore();
  const [lookupName, setLookupName] = useState("");
  const players = telemetry?.online_players ?? [];

  const lookup = (e: React.FormEvent) => {
    e.preventDefault();
    const name = lookupName.trim();
    if (!name) return;
    openPlayerInventory({
      username: name,
      uuid: "latest",
      is_op: false,
      gamemode: "survival",
      ping_ms: 0,
      joined_at_ms: Date.now(),
      coords: [0, 64, 0],
    });
  };

  return (
    <Page title="Players" subtitle="Help out your friends, give them items, or keep things fair.">
      {players.length === 0 ? (
        <Card>
          <EmptyState
            icon={Users}
            title="No one's online right now"
            text={
              status === "running"
                ? "When friends join your world, they'll appear here."
                : "Start your world from Home, then invite some friends."
            }
          />
        </Card>
      ) : (
        <div className="flex flex-col gap-3">
          {players.map((p) => (
            <PlayerCard key={p.username} player={p} />
          ))}
        </div>
      )}

      <Card title="Check someone's inventory" hint="Works for players who are offline too.">
        <form onSubmit={lookup} className="flex gap-2">
          <TextInput
            value={lookupName}
            onChange={(e) => setLookupName(e.target.value)}
            placeholder="Player name"
            aria-label="Player name"
          />
          <Button type="submit" disabled={!lookupName.trim()} icon={<Package className="w-4 h-4" />}>
            Open
          </Button>
        </form>
      </Card>

      {activeInventory && <InventoryModal />}
    </Page>
  );
};

/* ---------- One player ---------- */

const PlayerCard: React.FC<{ player: OnlinePlayer }> = ({ player }) => {
  const { executePlayerAction, openPlayerInventory } = useServerStore();
  const [showMore, setShowMore] = useState(false);
  const [tpTarget, setTpTarget] = useState("");
  const [confirmBan, setConfirmBan] = useState(false);
  const act = (action: string, payload?: string) =>
    executePlayerAction(player.username, action, payload);

  useEffect(() => {
    if (!confirmBan) return;
    const t = setTimeout(() => setConfirmBan(false), 4000);
    return () => clearTimeout(t);
  }, [confirmBan]);

  return (
    <section className="rounded-3xl bg-surface p-5">
      <div className="flex items-center gap-4">
        <PlayerHead name={player.username} size={52} />
        <div className="flex-1 min-w-0">
          <div className="text-lg font-bold truncate">
            {player.username} {player.is_op && <Crown className="inline w-4 h-4 text-sun -mt-0.5" aria-label="Admin" />}
          </div>
          <div className="text-sm text-soft">
            <span className="capitalize">{player.gamemode}</span> · {player.ping_ms} ms
          </div>
        </div>
        <Button variant="ghost" onClick={() => act("heal_feed")} icon={<Heart className="w-4 h-4" />}>
          Heal
        </Button>
        <Button onClick={() => openPlayerInventory(player)} icon={<Package className="w-4 h-4" />}>
          Inventory
        </Button>
        <Button
          variant="ghost"
          onClick={() => setShowMore(!showMore)}
          aria-expanded={showMore}
          aria-label="More actions"
          className="px-3"
        >
          <MoreHorizontal className="w-5 h-5" />
        </Button>
      </div>

      {showMore && (
        <div className="mt-5 pt-5 border-t border-line/60 grid grid-cols-1 md:grid-cols-2 gap-5">
          <div>
            <div className="text-sm font-bold text-soft mb-2">Game mode</div>
            <Segmented
              value={player.gamemode}
              onChange={(gm) => act("gamemode", gm)}
              options={[
                { id: "survival", label: "Survival" },
                { id: "creative", label: "Creative" },
                { id: "spectator", label: "Spectator" },
              ]}
            />
          </div>

          <div>
            <div className="text-sm font-bold text-soft mb-2">Teleport to</div>
            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                if (tpTarget.trim()) act("tp", tpTarget.trim());
              }}
            >
              <TextInput
                value={tpTarget}
                onChange={(e) => setTpTarget(e.target.value)}
                placeholder="Friend's name or X Y Z"
                aria-label="Teleport target"
              />
              <Button type="submit" disabled={!tpTarget.trim()} icon={<Compass className="w-4 h-4" />}>
                Go
              </Button>
            </form>
          </div>

          <div className="md:col-span-2 flex flex-wrap gap-2">
            <Button size="sm" onClick={() => act(player.is_op ? "deop" : "op")}>
              {player.is_op ? "Remove admin" : "Make admin"}
            </Button>
            <Button size="sm" onClick={() => act("kill")}>
              Respawn
            </Button>
            <Button size="sm" onClick={() => act("clear")}>
              Empty inventory
            </Button>
            <span className="flex-1" />
            <Button size="sm" variant="ghost" onClick={() => act("kick", "Kicked by the host")}>
              Kick
            </Button>
            <Button
              size="sm"
              variant="danger"
              onClick={() => {
                if (confirmBan) {
                  setConfirmBan(false);
                  act("ban", "Banned by the host");
                } else {
                  setConfirmBan(true);
                }
              }}
            >
              {confirmBan ? "Click again to ban" : "Ban"}
            </Button>
          </div>
        </div>
      )}
    </section>
  );
};

/* ---------- Inventory window ---------- */

const ARMOR_SLOTS = [
  { slot: 103, label: "Head" },
  { slot: 102, label: "Chest" },
  { slot: 101, label: "Legs" },
  { slot: 100, label: "Feet" },
  { slot: -106, label: "Off-hand" },
];

const nbtSlotToCommandSlot = (slot: number): string => {
  if (slot >= 0 && slot <= 8) return `hotbar.${slot}`;
  if (slot >= 9 && slot <= 35) return `container.${slot - 9}`;
  if (slot === 100) return "armor.feet";
  if (slot === 101) return "armor.legs";
  if (slot === 102) return "armor.chest";
  if (slot === 103) return "armor.head";
  if (slot === -106) return "weapon.offhand";
  return `container.${Math.max(0, slot)}`;
};

const InventoryModal: React.FC = () => {
  const { activeInventory: inv, executePlayerAction, closePlayerInventory } = useServerStore();
  const [itemSearch, setItemSearch] = useState("");
  const [amount, setAmount] = useState(1);
  const [customId, setCustomId] = useState("");

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && closePlayerInventory();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!inv) return null;
  const name = inv.username ?? "Player";

  const slotItem = (n: number) => inv.inventory.find((s) => s.slot === n) ?? null;
  const removeItem = (item: InventoryItemSlot, slot: number) =>
    inv.username &&
    executePlayerAction(inv.username, "delete_slot", item.command_slot || nbtSlotToCommandSlot(slot));
  const give = (id: string, count: number) => executePlayerAction(name, "give_item", `${id} ${count}`);

  const q = itemSearch.toLowerCase();
  const items = POPULAR_MINECRAFT_ITEMS.filter(
    (i) => i.name.toLowerCase().includes(q) || i.category.includes(q)
  );

  const Slot: React.FC<{ n: number; label?: string }> = ({ n, label }) => {
    const item = slotItem(n);
    const vis = item ? resolveItemVisual(item.id) : null;
    return (
      <button
        onClick={() => item && removeItem(item, n)}
        title={item && vis ? `${vis.label} ×${item.count} — click to remove` : label ?? "Empty"}
        className={`group relative aspect-square rounded-xl flex items-center justify-center transition-colors ${
          item ? "bg-raised hover:bg-danger/20 cursor-pointer" : "bg-raised/50 cursor-default"
        }`}
      >
        {item && vis ? (
          <>
            <vis.icon className="w-6 h-6 text-ink" />
            {item.count > 1 && (
              <span className="absolute bottom-0.5 right-1.5 text-xs font-extrabold">{item.count}</span>
            )}
            <X className="absolute top-1 right-1 w-3 h-3 text-danger opacity-0 group-hover:opacity-100" />
          </>
        ) : (
          label && <span className="text-[11px] font-bold text-faint">{label}</span>
        )}
      </button>
    );
  };

  return (
    <div
      className="fixed inset-0 z-40 bg-black/60 flex items-center justify-center p-6"
      onClick={closePlayerInventory}
    >
      <div
        role="dialog"
        aria-label={`${name}'s inventory`}
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-5xl max-h-[90vh] overflow-y-auto rounded-3xl bg-surface shadow-2xl"
      >
        <div className="flex items-center gap-4 p-6 pb-2">
          <PlayerHead name={name} size={48} />
          <div className="flex-1">
            <h2 className="text-xl font-extrabold">{name}'s inventory</h2>
            <div className="text-sm text-soft mt-0.5 flex flex-wrap items-center gap-x-4 gap-y-1">
              <span className="inline-flex items-center gap-1.5">
                <Heart className="w-4 h-4 text-danger" /> {inv.health.toFixed(0)}/20
              </span>
              <span className="inline-flex items-center gap-1.5">
                <Drumstick className="w-4 h-4 text-sun" /> {inv.food_level}/20
              </span>
              <span className="inline-flex items-center gap-1.5">
                <Sparkles className="w-4 h-4 text-brand" /> Level {inv.xp_level}
              </span>
              <span className="capitalize">{inv.dimension.replace("minecraft:", "").replace("_", " ")}</span>
            </div>
          </div>
          <Button variant="ghost" onClick={closePlayerInventory} aria-label="Close" className="px-3">
            <X className="w-5 h-5" />
          </Button>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-[1.25fr_1fr] gap-6 p-6">
          {/* Their items */}
          <div className="flex flex-col gap-4">
            <p className="text-sm text-soft">Click an item to take it away.</p>
            <div className="grid grid-cols-9 gap-1.5">
              {ARMOR_SLOTS.map((a) => (
                <Slot key={a.slot} n={a.slot} label={a.label} />
              ))}
            </div>
            <div className="grid grid-cols-9 gap-1.5">
              {Array.from({ length: 27 }, (_, i) => i + 9).map((n) => (
                <Slot key={n} n={n} />
              ))}
            </div>
            <div className="grid grid-cols-9 gap-1.5 pt-3 border-t border-line/60">
              {Array.from({ length: 9 }, (_, i) => i).map((n) => (
                <Slot key={n} n={n} label={String(n + 1)} />
              ))}
            </div>
          </div>

          {/* Give items */}
          <div className="rounded-2xl bg-bg/40 p-5 flex flex-col gap-3">
            <h3 className="font-bold text-lg flex items-center gap-2">
              <Gift className="w-5 h-5 text-soft" /> Give an item
            </h3>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Search className="w-4 h-4 text-faint absolute left-3.5 top-3.5" />
                <TextInput
                  value={itemSearch}
                  onChange={(e) => setItemSearch(e.target.value)}
                  placeholder="Search items"
                  className="pl-10"
                  aria-label="Search items"
                />
              </div>
              <Segmented
                value={String(amount)}
                onChange={(v) => setAmount(Number(v))}
                options={[
                  { id: "1", label: "1" },
                  { id: "16", label: "16" },
                  { id: "64", label: "64" },
                ]}
              />
            </div>
            <div className="grid grid-cols-2 gap-1.5 max-h-72 overflow-y-auto pr-1">
              {items.map((m) => (
                <button
                  key={m.id}
                  onClick={() => give(m.id, Math.min(amount, m.maxStack))}
                  className="flex items-center gap-2.5 rounded-xl px-3 py-2.5 text-left hover:bg-raised transition-colors cursor-pointer"
                >
                  <m.icon className="w-5 h-5 text-soft shrink-0" />
                  <span className="font-bold text-sm truncate">{m.name}</span>
                </button>
              ))}
              {items.length === 0 && (
                <div className="col-span-2 text-sm text-faint py-4 text-center">No items match.</div>
              )}
            </div>
            <div className="pt-2">
              <Disclosure label="Item from a mod">
                <form
                  className="flex gap-2"
                  onSubmit={(e) => {
                    e.preventDefault();
                    if (customId.trim()) give(customId.trim(), amount);
                  }}
                >
                  <TextInput
                    value={customId}
                    onChange={(e) => setCustomId(e.target.value)}
                    placeholder="modname:item_name"
                    className="font-mono text-sm"
                    aria-label="Item ID"
                  />
                  <Button type="submit" disabled={!customId.trim()}>
                    Give
                  </Button>
                </form>
              </Disclosure>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
