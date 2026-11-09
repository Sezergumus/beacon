import React, { useEffect, useState } from "react";
import {
  Compass,
  Crown,
  Drumstick,
  Footprints,
  Gift,
  HardHat,
  Heart,
  MoreHorizontal,
  Package,
  Search,
  Shield,
  Shirt,
  Sparkles,
  Users,
  X,
  type LucideIcon,
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

const ARMOR_SLOTS: { slot: number; label: string; icon: LucideIcon }[] = [
  { slot: 103, label: "Head", icon: HardHat },
  { slot: 102, label: "Chest", icon: Shirt },
  { slot: 101, label: "Legs", icon: Shirt },
  { slot: 100, label: "Feet", icon: Footprints },
  { slot: -106, label: "Off-hand", icon: Shield },
];

/** Only the border of an item's rarity colour is used, so slots keep the app palette. */
const rarityBorder = (rarityColor: string): string =>
  rarityColor.match(/border-\S+/)?.[0] ?? "border-line/60";

const SlotButton: React.FC<{
  item: InventoryItemSlot | null;
  label?: string;
  emptyIcon?: LucideIcon;
  onRemove: (item: InventoryItemSlot) => void;
}> = ({ item, label, emptyIcon: EmptyIcon, onRemove }) => {
  const vis = item ? resolveItemVisual(item.id) : null;
  if (!item || !vis) {
    return (
      <div
        title={label ?? "Empty"}
        className="aspect-square rounded-xl bg-bg/40 border border-line/30 flex items-center justify-center"
      >
        {EmptyIcon ? (
          <EmptyIcon className="w-5 h-5 text-faint/40" />
        ) : (
          label && <span className="text-[11px] font-bold text-faint/50">{label}</span>
        )}
      </div>
    );
  }
  return (
    <button
      onClick={() => onRemove(item)}
      title={`${vis.label} ×${item.count}: click to remove`}
      aria-label={`Remove ${vis.label}`}
      className={`group relative aspect-square rounded-xl border bg-raised flex items-center justify-center cursor-pointer transition-colors hover:bg-danger/20 hover:border-danger/60 ${rarityBorder(
        vis.rarityColor
      )}`}
    >
      <vis.icon className="w-6 h-6 text-ink group-hover:opacity-30 transition-opacity" />
      <X className="absolute w-5 h-5 text-danger opacity-0 group-hover:opacity-100 transition-opacity" />
      {item.count > 1 && (
        <span className="absolute bottom-0.5 right-1 rounded-md bg-bg/80 px-1 text-[11px] leading-4 font-extrabold tabular-nums">
          {item.count}
        </span>
      )}
    </button>
  );
};

const Stat: React.FC<{ icon: React.ReactNode; children: React.ReactNode }> = ({ icon, children }) => (
  <span className="inline-flex items-center gap-1.5 rounded-full bg-raised px-3 py-1 text-sm font-bold">
    {icon}
    {children}
  </span>
);

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

  const slot = (n: number, label?: string, emptyIcon?: LucideIcon) => (
    <SlotButton
      key={n}
      item={slotItem(n)}
      label={label}
      emptyIcon={emptyIcon}
      onRemove={(item) => removeItem(item, n)}
    />
  );
  const itemCount = inv.inventory.length;

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
          <PlayerHead name={name} size={56} />
          <div className="flex-1 min-w-0">
            <h2 className="text-xl font-extrabold truncate">{name}'s inventory</h2>
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <Stat icon={<Heart className="w-4 h-4 text-danger" />}>{inv.health.toFixed(0)}/20</Stat>
              <Stat icon={<Drumstick className="w-4 h-4 text-sun" />}>{inv.food_level}/20</Stat>
              <Stat icon={<Sparkles className="w-4 h-4 text-brand" />}>Level {inv.xp_level}</Stat>
              <Stat icon={<Compass className="w-4 h-4 text-info" />}>
                <span className="capitalize">
                  {inv.dimension.replace("minecraft:", "").replace(/_/g, " ")}
                </span>
              </Stat>
            </div>
          </div>
          <Button variant="ghost" onClick={closePlayerInventory} aria-label="Close" className="px-3 self-start">
            <X className="w-5 h-5" />
          </Button>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-[1.25fr_1fr] gap-6 p-6">
          {/* Their items */}
          <div className="rounded-2xl bg-bg/40 p-5 flex flex-col gap-5 self-start">
            <div className="flex items-center justify-between">
              <h3 className="font-bold text-lg flex items-center gap-2">
                <Package className="w-5 h-5 text-soft" /> Carrying
              </h3>
              <span className="text-sm text-soft">
                {itemCount === 0 ? "Nothing yet" : `${itemCount} ${itemCount === 1 ? "stack" : "stacks"}`}
              </span>
            </div>

            <section>
              <div className="mb-2 text-xs font-bold uppercase tracking-wide text-faint">Worn</div>
              <div className="grid grid-cols-5 gap-2 max-w-sm">
                {ARMOR_SLOTS.map((a) => slot(a.slot, a.label, a.icon))}
              </div>
            </section>

            <section>
              <div className="mb-2 text-xs font-bold uppercase tracking-wide text-faint">Backpack</div>
              <div className="grid grid-cols-9 gap-1.5">
                {Array.from({ length: 27 }, (_, i) => slot(i + 9))}
              </div>
            </section>

            <section>
              <div className="mb-2 text-xs font-bold uppercase tracking-wide text-faint">Hotbar</div>
              <div className="grid grid-cols-9 gap-1.5">
                {Array.from({ length: 9 }, (_, i) => slot(i, String(i + 1)))}
              </div>
            </section>

            <p className="text-sm text-faint">Click an item to take it away.</p>
          </div>

          {/* Give items */}
          <div className="rounded-2xl bg-bg/40 p-5 flex flex-col gap-3 self-start">
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
            <div className="grid grid-cols-2 gap-1.5 max-h-80 overflow-y-auto pr-1">
              {items.map((m) => (
                <button
                  key={m.id}
                  onClick={() => give(m.id, Math.min(amount, m.maxStack))}
                  className="flex items-center gap-2.5 rounded-xl bg-raised/40 px-3 py-2.5 text-left hover:bg-raised transition-colors cursor-pointer"
                >
                  <span className="w-8 h-8 shrink-0 rounded-lg bg-bg/50 flex items-center justify-center">
                    <m.icon className="w-4 h-4 text-soft" />
                  </span>
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
