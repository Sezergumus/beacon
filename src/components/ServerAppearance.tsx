import React, { useCallback, useEffect, useRef, useState } from "react";
import { ImagePlus, Loader2, Trash2 } from "lucide-react";
import { formatAppError, tauriApi } from "../lib/tauri";
import { useServerStore } from "../stores/useServerStore";
import {
  MOTD_COLORS,
  MOTD_FORMATS,
  MOTD_RESET,
  parseMotd,
  type MotdSpan,
} from "../lib/motd";
import { Button, TextInput } from "./ui";

const MAX_LINE_CHARS = 120;

/* ---------- Preview ---------- */

const spanStyle = (s: MotdSpan): React.CSSProperties => ({
  color: s.color ?? "#AAAAAA",
  fontWeight: s.bold ? 800 : 500,
  fontStyle: s.italic ? "italic" : "normal",
  textDecoration: [s.underline && "underline", s.strike && "line-through"].filter(Boolean).join(" ") || "none",
});

/** How the world shows up in Minecraft's multiplayer list. */
const ServerPreview: React.FC<{
  icon: string | null;
  motd: string;
  name: string;
  maxPlayers: number;
}> = ({ icon, motd, name, maxPlayers }) => {
  const lines = parseMotd(motd);
  return (
    <div
      className="rounded-2xl bg-black/70 p-3 flex gap-3 select-none"
      aria-label="Preview of the server list entry"
    >
      {icon ? (
        <img
          src={icon}
          alt=""
          className="w-16 h-16 shrink-0"
          style={{ imageRendering: "pixelated" }}
        />
      ) : (
        <div className="w-16 h-16 shrink-0 bg-raised/60 flex items-center justify-center text-faint text-xs font-bold">
          No icon
        </div>
      )}
      <div className="min-w-0 flex-1 font-mono text-[13px] leading-[1.35]">
        <div className="flex items-center justify-between gap-3 text-white">
          <span className="truncate font-bold">{name}</span>
          <span className="shrink-0 text-[#AAAAAA]">
            <span className="text-[#555555]">0</span>/{maxPlayers}
          </span>
        </div>
        {[0, 1].map((i) => (
          <div key={i} className="truncate h-[1.35em] whitespace-pre">
            {lines[i]?.map((s, j) => (
              <span key={j} style={spanStyle(s)}>
                {s.obfuscated ? "▒".repeat(s.text.length) : s.text}
              </span>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
};

/* ---------- MOTD editor ---------- */

export const MotdEditor: React.FC<{
  value: string;
  onChange: (next: string) => void;
  /** Called when the player leaves the boxes, e.g. to save right away. */
  onCommit?: (value: string) => void;
}> = ({ value, onChange, onCommit }) => {
  const [first, ...rest] = value.split("\n");
  const second = rest.join("\n");
  const inputs = [useRef<HTMLInputElement>(null), useRef<HTMLInputElement>(null)];
  const lastFocused = useRef(0);
  const latest = useRef(value);
  latest.current = value;

  const setLine = (index: number, text: string) => {
    const next = index === 0 ? [text, second] : [first, text];
    // An empty second line is simply not stored.
    onChange(next[1] === "" ? next[0] : next.join("\n"));
  };

  /** Put a code at the cursor (or around the selection) of the line being edited. */
  const insert = (code: string) => {
    const index = lastFocused.current;
    const el = inputs[index].current;
    const text = index === 0 ? first : second;
    const start = el?.selectionStart ?? text.length;
    const end = el?.selectionEnd ?? start;
    const next = `${text.slice(0, start)}§${code}${text.slice(start)}`;
    if (next.length > MAX_LINE_CHARS) return;
    setLine(index, next);
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(start + 2, end + 2);
    });
  };

  const commit = () => onCommit?.(latest.current);
  // Keep the cursor in the box when a toolbar button is pressed.
  const keepFocus = (e: React.MouseEvent) => e.preventDefault();

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-2">
        {[first, second].map((text, i) => (
          <TextInput
            key={i}
            ref={inputs[i]}
            value={text}
            maxLength={MAX_LINE_CHARS}
            onChange={(e) => setLine(i, e.target.value.replace(/[\r\n]/g, ""))}
            onFocus={() => (lastFocused.current = i)}
            onBlur={commit}
            placeholder={i === 0 ? "First line" : "Second line (optional)"}
            aria-label={i === 0 ? "Welcome message, first line" : "Welcome message, second line"}
            className="h-10 bg-surface font-mono text-sm"
          />
        ))}
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <div className="flex flex-wrap gap-1" role="group" aria-label="Colours">
          {MOTD_COLORS.map((c) => (
            <button
              key={c.code}
              type="button"
              title={c.name}
              aria-label={c.name}
              onMouseDown={keepFocus}
              onClick={() => insert(c.code)}
              className="w-6 h-6 rounded-md ring-1 ring-white/15 hover:scale-110 transition-transform cursor-pointer"
              style={{ backgroundColor: c.hex }}
            />
          ))}
        </div>
        <div className="flex gap-1" role="group" aria-label="Text style">
          {MOTD_FORMATS.map((f) => (
            <button
              key={f.code}
              type="button"
              title={f.name}
              aria-label={f.name}
              onMouseDown={keepFocus}
              onClick={() => insert(f.code)}
              className="w-7 h-7 rounded-md bg-surface text-sm font-bold text-soft hover:text-ink hover:bg-line cursor-pointer"
            >
              {f.label}
            </button>
          ))}
          <button
            type="button"
            onMouseDown={keepFocus}
            onClick={() => insert(MOTD_RESET)}
            className="h-7 px-2 rounded-md bg-surface text-xs font-bold text-soft hover:text-ink hover:bg-line cursor-pointer"
          >
            Reset
          </button>
        </div>
      </div>
    </div>
  );
};

/* ---------- Icon ---------- */

/** Centre-crop whatever picture was chosen to the 64x64 PNG Minecraft wants. */
const toIconPng = (file: File): Promise<string> =>
  new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      const side = Math.min(img.width, img.height);
      if (!side) return reject(new Error("empty picture"));
      const canvas = document.createElement("canvas");
      canvas.width = 64;
      canvas.height = 64;
      const ctx = canvas.getContext("2d");
      if (!ctx) return reject(new Error("no canvas"));
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(img, (img.width - side) / 2, (img.height - side) / 2, side, side, 0, 0, 64, 64);
      resolve(canvas.toDataURL("image/png").split(",")[1]);
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("unreadable picture"));
    };
    img.src = url;
  });

const IconPicker: React.FC<{
  icon: string | null;
  busy: boolean;
  onPick: (png: string) => void;
  onRemove: () => void;
}> = ({ icon, busy, onPick, onRemove }) => {
  const input = useRef<HTMLInputElement>(null);
  const pushToast = useServerStore((s) => s.pushToast);

  const onFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    try {
      onPick(await toIconPng(file));
    } catch {
      pushToast("error", "Couldn't use that picture", "Try a PNG, JPG or WebP image.");
    }
  };

  return (
    <div className="flex items-center gap-2">
      <input
        ref={input}
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        className="hidden"
        onChange={onFile}
      />
      <Button
        size="sm"
        onClick={() => input.current?.click()}
        disabled={busy}
        icon={busy ? <Loader2 className="w-4 h-4 animate-spin" /> : <ImagePlus className="w-4 h-4" />}
      >
        {icon ? "Change picture" : "Choose picture"}
      </Button>
      {icon && (
        <Button size="sm" variant="ghost" onClick={onRemove} disabled={busy} icon={<Trash2 className="w-4 h-4" />}>
          Remove
        </Button>
      )}
    </div>
  );
};

/* ---------- Together ---------- */

/**
 * Icon, welcome message and a live preview of both. The icon is saved as soon as it is
 * picked; the welcome message is handed to the parent, which decides when to save it.
 */
export const ServerAppearance: React.FC<{
  /** The world, or `null` for the icon and message new worlds start with. */
  worldId: string | null;
  motd: string;
  onMotdChange: (next: string) => void;
  onMotdCommit?: (value: string) => void;
  /** Name shown in the preview. */
  name?: string;
  maxPlayers?: number;
}> = ({ worldId, motd, onMotdChange, onMotdCommit, name = "Minecraft Server", maxPlayers = 16 }) => {
  const pushToast = useServerStore((s) => s.pushToast);
  const [icon, setIcon] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    tauriApi
      .getServerIcon(worldId)
      .then((i) => !cancelled && setIcon(i))
      .catch(() => !cancelled && setIcon(null));
    return () => {
      cancelled = true;
    };
  }, [worldId]);

  const run = useCallback(
    async (action: () => Promise<void>, done: string) => {
      setBusy(true);
      try {
        await action();
        setIcon(await tauriApi.getServerIcon(worldId));
        pushToast("success", done, worldId ? "Friends see it after the world restarts." : "New worlds start with it.");
      } catch (err) {
        pushToast("error", "Couldn't change the icon", formatAppError(err));
      } finally {
        setBusy(false);
      }
    },
    [worldId, pushToast],
  );

  return (
    <div className="flex flex-col gap-4">
      <ServerPreview icon={icon} motd={motd} name={name} maxPlayers={maxPlayers} />
      <div>
        <div className="font-bold">Icon</div>
        <div className="text-sm text-soft mt-0.5 mb-2">
          Shown next to the world in Minecraft's server list. Pictures are cropped to a square.
        </div>
        <IconPicker
          icon={icon}
          busy={busy}
          onPick={(png) => run(() => tauriApi.setServerIcon(worldId, png), "Icon saved")}
          onRemove={() => run(() => tauriApi.clearServerIcon(worldId), "Icon removed")}
        />
      </div>
      <div>
        <div className="font-bold">Welcome message</div>
        <div className="text-sm text-soft mt-0.5 mb-2">
          Two lines under the name in the server list. Pick a colour, then type.
        </div>
        <MotdEditor value={motd} onChange={onMotdChange} onCommit={onMotdCommit} />
      </div>
    </div>
  );
};
