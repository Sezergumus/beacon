import React, { useEffect, useMemo } from "react";
import { useServerStore } from "../../stores/useServerStore";
import type { ServerCore } from "../../lib/tauri";
import { Segmented } from "../ui";

const VERSION_NOTES: Record<string, string> = {
  "1.21.1": "most popular",
  "1.20.1": "best for mods",
  "1.16.5": "classic modpacks",
  "1.12.2": "classic modpacks",
  "1.8.8": "PvP classic",
};

/** `1.21.4` and `1.21` belong to the group "1.21"; year-based `26.1.2` to "26.1". */
const versionGroup = (v: string) => v.split(".").slice(0, 2).join(".");

/** Version and server type pickers for a world that is about to be made. */
export const VersionFields: React.FC<{
  core: ServerCore;
  version: string;
  onChange: (core: ServerCore, version: string) => void;
}> = ({ core, version, onChange }) => {
  const list = useServerStore((s) => s.versions[core]);
  const loadVersions = useServerStore((s) => s.loadVersions);

  useEffect(() => {
    loadVersions(core);
  }, [core, loadVersions]);

  // Each server type supports different versions: move to a sensible one if this one isn't offered.
  useEffect(() => {
    if (!list || list.includes(version)) return;
    onChange(core, list.includes("1.21.1") ? "1.21.1" : list[0]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [list, core, version]);

  const groups = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const v of list ?? []) {
      const g = versionGroup(v);
      map.set(g, [...(map.get(g) ?? []), v]);
    }
    return [...map.entries()];
  }, [list]);
  const newest = list?.[0];

  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
      <label className="flex items-center gap-3">
        <span className="font-bold text-soft">Version</span>
        <select
          value={version}
          onChange={(e) => onChange(core, e.target.value)}
          disabled={!list}
          className="h-11 rounded-xl bg-raised px-3 font-bold text-ink cursor-pointer focus:outline-none focus:ring-2 focus:ring-brand/60 disabled:opacity-50"
        >
          {!list && <option value={version}>Loading versions…</option>}
          {groups.map(([group, items]) => (
            <optgroup key={group} label={`${group}.x`}>
              {items.map((v) => (
                <option key={v} value={v}>
                  {v}
                  {v === newest ? " (newest)" : VERSION_NOTES[v] ? ` (${VERSION_NOTES[v]})` : ""}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </label>
      <div
        className="flex items-center gap-3"
        title={
          core === "paper"
            ? "Classic Minecraft with plugin support."
            : "Pick this if you want to add mods."
        }
      >
        <span className="font-bold text-soft">Type</span>
        <Segmented<ServerCore>
          value={core}
          onChange={(c) => onChange(c, version)}
          options={[
            { id: "paper", label: "Paper" },
            { id: "fabric", label: "Fabric" },
            { id: "neoforge", label: "NeoForge" },
          ]}
        />
      </div>
    </div>
  );
};
