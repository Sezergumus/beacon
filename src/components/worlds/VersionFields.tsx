import React, { useEffect, useMemo } from "react";
import { useServerStore } from "../../stores/useServerStore";
import type { ServerCore } from "../../lib/tauri";
import { Segmented } from "../ui";
import { Select, type SelectOption } from "../Select";

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

  const options = useMemo<SelectOption<string>[]>(
    () =>
      (list ?? []).map((v, i) => ({
        value: v,
        label: v,
        group: `${versionGroup(v)}.x`,
        hint: i === 0 ? "newest" : VERSION_NOTES[v],
      })),
    [list],
  );

  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
      <div className="flex items-center gap-3">
        <span className="font-bold text-soft">Version</span>
        <Select<string>
          aria-label="Version"
          className="w-52"
          value={version}
          options={options}
          placeholder="Loading versions…"
          disabled={!list}
          onChange={(v) => onChange(core, v)}
        />
      </div>
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
