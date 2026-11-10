import React, { useState } from "react";
import { resolveItemVisual } from "../lib/minecraft-items";

const TEXTURE_BASE =
  "https://cdn.jsdelivr.net/gh/PrismarineJS/minecraft-assets@master/data/1.21.1";

/** Items whose texture has a different name from their id. */
const TEXTURE_ALIAS: Record<string, string> = {
  enchanted_golden_apple: "golden_apple",
};

/** Texture URLs to try in order: flat item sprite first, then the block texture. */
const textureUrls = (itemId: string): string[] => {
  if (itemId.includes(":") && !itemId.startsWith("minecraft:")) return []; // modded item
  const short = itemId.replace(/^minecraft:/, "").toLowerCase();
  const name = TEXTURE_ALIAS[short] ?? short;
  return [`${TEXTURE_BASE}/items/${name}.png`, `${TEXTURE_BASE}/blocks/${name}.png`];
};

// Remember textures that don't exist so they aren't requested again.
const missing = new Set<string>();

export const ItemIcon: React.FC<{ id: string; className?: string }> = ({ id, className = "w-6 h-6" }) => {
  const [failed, setFailed] = useState(0);
  const url = textureUrls(id)
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
