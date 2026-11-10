// Builds src/lib/minecraft-item-list.json: every item as [id, display name, stack size, category, icon].
// `icon` says where the picture is: "i" = PrismarineJS items/ sprite, "b" = its blocks/ texture,
// "Invicon_X.png" = Minecraft Wiki inventory icon (for items drawn from a 3D model), "" = none.
// Sources: PrismarineJS/minecraft-data and PrismarineJS/minecraft-assets. Run `npm run items:generate [version]` to refresh it.
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const VERSION = process.argv[2] ?? "26.1";
const BASE = `https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/pc/${VERSION}`;

const getJson = async (file) => {
  const res = await fetch(`${BASE}/${file}`);
  if (!res.ok) throw new Error(`Couldn't fetch ${BASE}/${file}: HTTP ${res.status}`);
  return res.json();
};

const ASSETS = `https://api.github.com/repos/PrismarineJS/minecraft-assets/git/trees/master:data%2F${VERSION}`;
const pngNames = async (dir) => {
  const res = await fetch(`${ASSETS}%2F${dir}`);
  if (!res.ok) throw new Error(`Couldn't list ${dir}: HTTP ${res.status}`);
  const tree = (await res.json()).tree;
  return new Set(tree.filter((f) => f.path.endsWith(".png")).map((f) => f.path.slice(0, -4)));
};
const itemSprites = await pngNames("items");
const blockSprites = await pngNames("blocks");

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

const all = items.filter((i) => i.name !== "air");
const displayOf = new Map(all.map((i) => [i.name, i.displayName]));

// Waxed and infested variants look exactly like the plain item and have no icon of their own.
const wikiFile = (name) => {
  const base = name.replace(/^(waxed_|infested_)/, "");
  const label = displayOf.get(base) ?? displayOf.get(name);
  return `Invicon_${label.replace(/ /g, "_")}.png`;
};

const needWiki = all.filter((i) => !itemSprites.has(i.name) && !blockSprites.has(i.name));
const wikiExists = new Set();
for (let n = 0; n < needWiki.length; n += 40) {
  const files = [...new Set(needWiki.slice(n, n + 40).map((i) => wikiFile(i.name)))];
  const api = new globalThis.URL("https://minecraft.wiki/api.php");
  api.search = new URLSearchParams({
    action: "query",
    format: "json",
    titles: files.map((f) => `File:${f}`).join("|"),
  });
  const res = await fetch(api, { headers: { "User-Agent": "Beacon-item-list-generator" } });
  if (!res.ok) throw new Error(`Wiki lookup failed: HTTP ${res.status}`);
  const pages = (await res.json()).query.pages;
  for (const page of Object.values(pages)) {
    if (page.missing === undefined && page.invalid === undefined) wikiExists.add(page.title.slice(5).replace(/ /g, "_"));
  }
}

const iconOf = (name) => {
  if (itemSprites.has(name)) return "i";
  if (blockSprites.has(name)) return "b";
  const file = wikiFile(name);
  return wikiExists.has(file) ? file : "";
};

const list = all.map((i) => [i.name, i.displayName, i.stackSize, categoryOf(i.name), iconOf(i.name)]);

const out = fileURLToPath(new URL("../src/lib/minecraft-item-list.json", import.meta.url));
writeFileSync(out, JSON.stringify(list) + "\n");

const counts = {};
for (const [, , , c] of list) counts[c] = (counts[c] ?? 0) + 1;
const icons = { sprite: 0, block: 0, wiki: 0, none: 0 };
for (const [, , , , ic] of list) icons[ic === "i" ? "sprite" : ic === "b" ? "block" : ic ? "wiki" : "none"]++;
console.log("icons", icons);
if (icons.none) console.log("no icon:", list.filter((x) => !x[4]).map((x) => x[0]).join(", "));
console.log(`Wrote ${list.length} items (Minecraft ${VERSION}) to ${out}`, counts);
