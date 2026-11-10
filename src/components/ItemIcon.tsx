import React, { useState } from "react";
import { getCatalogItem, resolveItemVisual } from "../lib/minecraft-items";

const SPRITES = "https://cdn.jsdelivr.net/gh/PrismarineJS/minecraft-assets@master/data/26.1";
const WIKI_ICONS = "https://minecraft.wiki/images";

/**
 * Picture URLs to try, best first.
 * Known vanilla items say where their picture is (see scripts/generate-item-list.mjs);
 * anything else vanilla gets a guess, and mod items get none.
 */
const iconUrls = (itemId: string): string[] => {
  const known = getCatalogItem(itemId);
  if (known) {
    const name = known.id.replace("minecraft:", "");
    if (known.icon === "i") return [`${SPRITES}/items/${name}.png`];
    if (known.icon === "b") return [`${SPRITES}/blocks/${name}.png`];
    return known.icon ? [`${WIKI_ICONS}/${known.icon}`] : [];
  }
  if (itemId.includes(":") && !itemId.startsWith("minecraft:")) return []; // modded item
  const name = itemId.replace(/^minecraft:/, "").toLowerCase();
  return [`${SPRITES}/items/${name}.png`, `${SPRITES}/blocks/${name}.png`];
};

// Remember pictures that failed to load so they aren't requested again.
const missing = new Set<string>();

export const ItemIcon: React.FC<{ id: string; className?: string }> = ({ id, className = "w-6 h-6" }) => {
  const [failed, setFailed] = useState(0);
  const url = iconUrls(id)
    .slice(failed)
    .find((u) => !missing.has(u));

  if (!url) {
    const Fallback = resolveItemVisual(id).icon;
    return <Fallback className={`${className} text-soft`} aria-hidden />;
  }
  return (
    <img
      src={url}
      alt=""
      draggable={false}
      loading="lazy"
      className={className}
      style={{ imageRendering: "pixelated" }}
      onError={() => {
        missing.add(url);
        setFailed((n) => n + 1);
      }}
    />
  );
};
