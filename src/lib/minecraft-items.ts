import {
  Apple,
  BookOpen,
  Box,
  Carrot,
  Coins,
  Crosshair,
  Drumstick,
  Feather,
  FlaskConical,
  Footprints,
  Gem,
  Hammer,
  HardHat,
  Axe,
  Package,
  Pickaxe,
  Rocket,
  Shield,
  Shirt,
  Shovel,
  Sparkles,
  Square,
  Sword,
  TreePine,
  CircleDot,
  type LucideIcon,
} from "lucide-react";

export interface MinecraftItemMeta {
  id: string;
  name: string;
  category: "combat" | "tools" | "armor" | "food" | "blocks" | "materials" | "utility";
  icon: LucideIcon;
  rarity: "common" | "uncommon" | "rare" | "epic";
  maxStack: number;
}

export const POPULAR_MINECRAFT_ITEMS: MinecraftItemMeta[] = [
  { id: "minecraft:netherite_sword", name: "Netherite Sword", category: "combat", icon: Sword, rarity: "epic", maxStack: 1 },
  { id: "minecraft:diamond_sword", name: "Diamond Sword", category: "combat", icon: Sword, rarity: "rare", maxStack: 1 },
  { id: "minecraft:mace", name: "Heavy Mace", category: "combat", icon: Hammer, rarity: "epic", maxStack: 1 },
  { id: "minecraft:bow", name: "Bow", category: "combat", icon: Crosshair, rarity: "common", maxStack: 1 },
  { id: "minecraft:totem_of_undying", name: "Totem of Undying", category: "combat", icon: Sparkles, rarity: "rare", maxStack: 1 },
  { id: "minecraft:shield", name: "Shield", category: "combat", icon: Shield, rarity: "common", maxStack: 1 },
  { id: "minecraft:netherite_pickaxe", name: "Netherite Pickaxe", category: "tools", icon: Pickaxe, rarity: "epic", maxStack: 1 },
  { id: "minecraft:diamond_pickaxe", name: "Diamond Pickaxe", category: "tools", icon: Pickaxe, rarity: "rare", maxStack: 1 },
  { id: "minecraft:netherite_axe", name: "Netherite Axe", category: "tools", icon: Axe, rarity: "epic", maxStack: 1 },
  { id: "minecraft:elytra", name: "Elytra Wings", category: "armor", icon: Feather, rarity: "epic", maxStack: 1 },
  { id: "minecraft:netherite_helmet", name: "Netherite Helmet", category: "armor", icon: HardHat, rarity: "epic", maxStack: 1 },
  { id: "minecraft:netherite_chestplate", name: "Netherite Chestplate", category: "armor", icon: Shirt, rarity: "epic", maxStack: 1 },
  { id: "minecraft:netherite_leggings", name: "Netherite Leggings", category: "armor", icon: Shirt, rarity: "epic", maxStack: 1 },
  { id: "minecraft:netherite_boots", name: "Netherite Boots", category: "armor", icon: Footprints, rarity: "epic", maxStack: 1 },
  { id: "minecraft:enchanted_golden_apple", name: "Enchanted Golden Apple", category: "food", icon: Apple, rarity: "epic", maxStack: 64 },
  { id: "minecraft:golden_apple", name: "Golden Apple", category: "food", icon: Apple, rarity: "rare", maxStack: 64 },
  { id: "minecraft:golden_carrot", name: "Golden Carrot", category: "food", icon: Carrot, rarity: "uncommon", maxStack: 64 },
  { id: "minecraft:cooked_beef", name: "Steak", category: "food", icon: Drumstick, rarity: "common", maxStack: 64 },
  { id: "minecraft:diamond", name: "Diamond", category: "materials", icon: Gem, rarity: "rare", maxStack: 64 },
  { id: "minecraft:netherite_ingot", name: "Netherite Ingot", category: "materials", icon: Coins, rarity: "epic", maxStack: 64 },
  { id: "minecraft:emerald", name: "Emerald", category: "materials", icon: Gem, rarity: "uncommon", maxStack: 64 },
  { id: "minecraft:iron_ingot", name: "Iron Ingot", category: "materials", icon: Coins, rarity: "common", maxStack: 64 },
  { id: "minecraft:ender_pearl", name: "Ender Pearl", category: "utility", icon: CircleDot, rarity: "uncommon", maxStack: 16 },
  { id: "minecraft:firework_rocket", name: "Firework Rocket", category: "utility", icon: Rocket, rarity: "common", maxStack: 64 },
  { id: "minecraft:shulker_box", name: "Shulker Box", category: "utility", icon: Package, rarity: "rare", maxStack: 1 },
  { id: "minecraft:experience_bottle", name: "Bottle o' Enchanting", category: "utility", icon: FlaskConical, rarity: "uncommon", maxStack: 64 },
  { id: "minecraft:obsidian", name: "Obsidian", category: "blocks", icon: Square, rarity: "uncommon", maxStack: 64 },
  { id: "minecraft:oak_log", name: "Oak Log", category: "blocks", icon: TreePine, rarity: "common", maxStack: 64 },
];

export function resolveItemVisual(itemId: string): {
  label: string;
  icon: LucideIcon;
  rarityColor: string;
} {
  const normalized = itemId.startsWith("minecraft:")
    ? itemId.toLowerCase()
    : `minecraft:${itemId.toLowerCase()}`;

  const exact = POPULAR_MINECRAFT_ITEMS.find((i) => i.id === normalized);
  if (exact) {
    const colorMap: Record<MinecraftItemMeta["rarity"], string> = {
      common: "border-slate-700 bg-slate-900/90 text-slate-200",
      uncommon: "border-emerald-500/40 bg-emerald-950/40 text-emerald-200",
      rare: "border-cyan-500/50 bg-cyan-950/40 text-cyan-200",
      epic: "border-purple-500/50 bg-purple-950/40 text-purple-200",
    };
    return {
      label: exact.name,
      icon: exact.icon,
      rarityColor: colorMap[exact.rarity],
    };
  }

  const short = normalized.replace("minecraft:", "");
  const pretty = short
    .split("_")
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");

  let icon: LucideIcon = Box;
  if (short.includes("sword") || short.includes("blade")) icon = Sword;
  else if (short.includes("pickaxe")) icon = Pickaxe;
  else if (short.includes("axe")) icon = Axe;
  else if (short.includes("shovel")) icon = Shovel;
  else if (short.includes("helmet") || short.includes("cap")) icon = HardHat;
  else if (short.includes("chestplate") || short.includes("tunic") || short.includes("leggings") || short.includes("pants")) icon = Shirt;
  else if (short.includes("boots")) icon = Footprints;
  else if (short.includes("apple") || short.includes("bread")) icon = Apple;
  else if (short.includes("beef")) icon = Drumstick;
  else if (short.includes("potion") || short.includes("bottle")) icon = FlaskConical;
  else if (short.includes("book")) icon = BookOpen;
  else if (short.includes("diamond")) icon = Gem;
  else if (short.includes("gold")) icon = Coins;

  const isValuable =
    short.includes("netherite") ||
    short.includes("diamond") ||
    short.includes("enchanted") ||
    short.includes("totem");

  return {
    label: pretty,
    icon,
    rarityColor: isValuable
      ? "border-cyan-500/50 bg-cyan-950/40 text-cyan-200"
      : "border-slate-700 bg-slate-900/90 text-slate-200",
  };
}
