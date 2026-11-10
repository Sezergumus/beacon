import React, { useState } from "react";
import { getCatalogItem, resolveItemVisual } from "../lib/minecraft-items";

/**
 * Item pictures ship with the app in public/items/<id>.png (made by `npm run items:generate`),
 * so they work offline. Items outside the vanilla list (mods, newer versions) get a line icon.
 */
export const ItemIcon: React.FC<{ id: string; className?: string }> = ({ id, className = "w-6 h-6" }) => {
  const [broken, setBroken] = useState(false);
  const known = getCatalogItem(id);

  if (!known || broken) {
    const Fallback = resolveItemVisual(id).icon;
    return <Fallback className={`${className} text-soft`} aria-hidden />;
  }
  return (
    <img
      src={`/items/${known.id.replace("minecraft:", "")}.png`}
      alt=""
      draggable={false}
      loading="lazy"
      className={className}
      style={{ imageRendering: "pixelated" }}
      onError={() => setBroken(true)}
    />
  );
};
