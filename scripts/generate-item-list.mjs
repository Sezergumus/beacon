// Builds src/lib/minecraft-item-list.json: every item as [id, display name, stack size, category].
// Source: PrismarineJS/minecraft-data. Run `npm run items:generate [version]` to refresh it.
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const VERSION = process.argv[2] ?? "26.1";
const BASE = `https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/pc/${VERSION}`;

const getJson = async (file) => {
  const res = await fetch(`${BASE}/${file}`);
  if (!res.ok) throw new Error(`Couldn't fetch ${BASE}/${file}: HTTP ${res.status}`);
  return res.json();
};

const items = await getJson("items.json");
const blockNames = new Set((await getJson("blocks.json")).map((b) => b.name));

const ARMOR = /(_helmet|_chestplate|_leggings|_boots|_horse_armor|_wolf_armor|^elytra$|^turtle_scute$)/;
const COMBAT = /(_sword|_spear|^bow$|^crossbow$|^trident$|^mace$|^arrow$|_arrow$|^shield$|^totem_of_undying$|^wind_charge$|^fire_charge$|^snowball$)/;
const TOOLS = /(_pickaxe|_axe|_shovel|_hoe|^shears$|^flint_and_steel$|^fishing_rod$|_on_a_stick$|^compass$|^recovery_compass$|^clock$|^brush$|^spyglass$|^lead$|^name_tag$|^saddle$|_bucket$|^bucket$|^bundle$|^map$|^filled_map$|^writable_book$)/;
const FOOD =
  /(apple|bread|beef|porkchop|mutton|chicken|rabbit|^cod$|salmon|potato|carrot|beetroot|melon_slice|cookie|^cake$|pumpkin_pie|_stew$|_soup$|honey_bottle|berries|dried_kelp|rotten_flesh|spider_eye|pufferfish|tropical_fish|chorus_fruit|^milk_bucket$|^egg$|suspicious_stew|^glow_berries$)/;

const NOT_FOOD = /(spawn_egg|music_disc|rabbit_foot|rabbit_hide|_seeds$|fermented_|popped_|glistering_)/;

const categoryOf = (name) => {
  if (ARMOR.test(name)) return "armor";
  if (COMBAT.test(name)) return "combat";
  if (TOOLS.test(name)) return "tools";
  if (FOOD.test(name) && !NOT_FOOD.test(name) && !blockNames.has(name)) return "food";
  if (blockNames.has(name)) return "blocks";
  return "other";
};

const list = items
  .filter((i) => i.name !== "air")
  .map((i) => [i.name, i.displayName, i.stackSize, categoryOf(i.name)]);

const out = fileURLToPath(new URL("../src/lib/minecraft-item-list.json", import.meta.url));
writeFileSync(out, JSON.stringify(list) + "\n");

const counts = {};
for (const [, , , c] of list) counts[c] = (counts[c] ?? 0) + 1;
console.log(`Wrote ${list.length} items (Minecraft ${VERSION}) to ${out}`, counts);
