import { useId } from "react";
import type { ServerStatus } from "../lib/tauri";

interface BeaconArtProps {
  variant?: "sidebar" | "card";
  status?: ServerStatus;
}

/** A transparent brand mark, with independently animated light on the card (see index.css). */
export function BeaconArt({ variant = "sidebar", status = "stopped" }: BeaconArtProps) {
  const lightFilter = `beacon-light-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;

  const src = variant === "sidebar" ? "/beacon-mark.png" : "/beacon-card-front.png";

  return (
    <span className={`beacon-art beacon-art--${variant}`} data-state={status} aria-hidden="true">
      {variant === "card" && (
        <svg width="0" height="0" className="absolute" focusable="false">
          <defs>
            <filter id={lightFilter} colorInterpolationFilters="sRGB">
              {/* Extract the teal channels, leaving the charcoal frame and pedestal unlit. */}
              <feColorMatrix type="matrix" values="0 0 0 0 0.24  0 0 0 0 0.86  0 0 0 0 0.85  -6 6 0 0 -0.2" />
            </filter>
          </defs>
        </svg>
      )}
      {variant === "card" && <span className="beacon-art__halo" />}
      <span className="beacon-art__body">
        <img src={src} alt="" width={variant === "sidebar" ? 44 : 96} height={variant === "sidebar" ? 44 : 96} draggable={false} />
        {variant === "card" && (
          <span className="beacon-art__light" style={{ filter: `url(#${lightFilter})` }}>
            <img src={src} alt="" width={96} height={96} draggable={false} />
          </span>
        )}
      </span>
    </span>
  );
}
