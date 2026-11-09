import React, { useEffect, useState } from "react";
import { Check, Download, Loader2, Puzzle, Search, Trash2 } from "lucide-react";
import { useServerStore } from "../../stores/useServerStore";
import { Button, Card, Disclosure, EmptyState, Page, ProgressBar, Segmented, TextInput } from "../ui";

const prettyFileName = (filename: string) =>
  filename
    .replace(/\.(jar|zip|mrpack)$/i, "")
    .replace(/[-_]?(mc)?\d+(\.\d+)+.*$/i, "")
    .replace(/[-_]+/g, " ")
    .trim() || filename;

const formatDownloads = (n: number) =>
  n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1000 ? `${Math.round(n / 1000)}k` : String(n);

export const ModsView: React.FC = () => {
  const {
    selectedCore,
    selectedVersion,
    searchResults,
    isSearchingMods,
    installingSlug,
    modpackProgress,
    installedContent,
    searchModrinth,
    installModrinthItem,
    installLocalMrpack,
    refreshInstalledContent,
    removeContentItem,
  } = useServerStore();

  const single = selectedCore === "paper" ? "plugin" : "mod";
  const [kind, setKind] = useState<"single" | "modpack">("single");
  const [query, setQuery] = useState("");
  const [mrpackPath, setMrpackPath] = useState("");

  const projectType = kind === "modpack" ? "modpack" : single;
  const noun = kind === "modpack" ? "modpacks" : `${single}s`;

  useEffect(() => {
    refreshInstalledContent();
    searchModrinth("", projectType);
  }, [selectedCore, selectedVersion, kind]);

  return (
    <Page
      title="Mods"
      subtitle={`Add fun stuff to your world. Friends get the same ${single}s automatically when they join.`}
    >
      <Card>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            searchModrinth(query, projectType);
          }}
        >
          <div className="relative flex-1">
            <Search className="w-4 h-4 text-faint absolute left-4 top-3.5" />
            <TextInput
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={`Search ${noun} for ${selectedVersion}`}
              className="pl-11"
              aria-label="Search"
            />
          </div>
          <Button type="submit" variant="primary" disabled={isSearchingMods}>
            {isSearchingMods ? "Searching…" : "Search"}
          </Button>
        </form>
        <div className="flex items-center justify-between gap-3 mt-4">
          <Segmented
            value={kind}
            onChange={setKind}
            options={[
              { id: "single", label: single === "plugin" ? "Plugins" : "Mods" },
              { id: "modpack", label: "Modpacks" },
            ]}
          />
          <span className="text-sm text-faint">
            Showing {noun} that work with {selectedCore === "paper" ? "Paper" : selectedCore === "fabric" ? "Fabric" : "NeoForge"} {selectedVersion}
          </span>
        </div>

        {modpackProgress && (
          <div className="mt-5">
            <ProgressBar
              percent={(modpackProgress.completed_files / Math.max(1, modpackProgress.total_files)) * 100}
              label={`Installing ${modpackProgress.pack_name}`}
              detail={`${modpackProgress.completed_files} of ${modpackProgress.total_files}`}
            />
          </div>
        )}
      </Card>

      {/* Results */}
      <div className="flex flex-col gap-2">
        {searchResults.length === 0 && !isSearchingMods && (
          <Card>
            <EmptyState icon={Search} title="Nothing found" text="Try a different word, like “maps” or “furniture”." />
          </Card>
        )}
        {searchResults.map((hit) => {
          const installing = installingSlug === hit.slug;
          return (
            <div key={hit.project_id} className="rounded-2xl bg-surface p-4 flex items-center gap-4">
              {hit.icon_url ? (
                <img src={hit.icon_url} alt="" className="w-14 h-14 rounded-xl object-cover bg-raised shrink-0" />
              ) : (
                <div className="w-14 h-14 rounded-xl bg-raised flex items-center justify-center text-soft shrink-0">
                  <Puzzle className="w-6 h-6" />
                </div>
              )}
              <div className="flex-1 min-w-0">
                <div className="flex items-baseline gap-2">
                  <h3 className="font-bold truncate">{hit.title}</h3>
                  <span className="text-xs text-faint shrink-0">{formatDownloads(hit.downloads)} downloads</span>
                </div>
                <p className="text-sm text-soft line-clamp-2 mt-0.5">{hit.description}</p>
              </div>
              <Button
                onClick={() => installModrinthItem(hit)}
                disabled={installing || installingSlug !== null}
                icon={installing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Download className="w-4 h-4" />}
              >
                {installing ? "Adding…" : "Add"}
              </Button>
            </div>
          );
        })}
      </div>

      {/* Installed */}
      <Card title={`In your world (${installedContent.length})`}>
        {installedContent.length === 0 ? (
          <p className="text-soft">Nothing added yet. Pick something above and press Add.</p>
        ) : (
          <ul className="flex flex-col">
            {installedContent.map((item) => (
              <li
                key={`${item.kind}-${item.filename}`}
                className="flex items-center gap-3 py-2.5 border-b border-line/60 last:border-0"
              >
                <Check className="w-4 h-4 text-brand shrink-0" />
                <span className="flex-1 min-w-0 font-bold truncate capitalize" title={item.filename}>
                  {prettyFileName(item.filename)}
                </span>
                <span className="text-xs text-faint capitalize">{item.kind}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => removeContentItem(item.filename, item.kind)}
                  aria-label={`Remove ${item.filename}`}
                  className="px-2.5"
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <div className="px-2 pb-4">
        <Disclosure label="Install a modpack file from my PC">
          <div className="flex gap-2">
            <TextInput
              value={mrpackPath}
              onChange={(e) => setMrpackPath(e.target.value)}
              placeholder="C:\Downloads\my-pack.mrpack"
              className="font-mono text-sm"
              aria-label="Modpack file path"
            />
            <Button
              onClick={() => installLocalMrpack(mrpackPath)}
              disabled={!mrpackPath.trim() || installingSlug !== null}
            >
              Install
            </Button>
          </div>
        </Disclosure>
      </div>
    </Page>
  );
};
