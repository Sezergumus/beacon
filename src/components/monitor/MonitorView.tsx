import React, { useEffect, useMemo, useRef, useState } from "react";
import { Send, Trash2 } from "lucide-react";
import { useServerStore } from "../../stores/useServerStore";
import { Button, Card, Page, Segmented, StatusDot, TextInput } from "../ui";
import { LiveChart } from "./LiveChart";
import { LogLine, parseLogEntry } from "../../lib/logFormat";

const formatUptime = (secs: number) => {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
};

const gb = (mb: number) => `${(mb / 1024).toFixed(1)} GB`;

const Tile: React.FC<{
  label: string;
  value: string;
  note?: string;
  noteTone?: "ok" | "warn" | "bad" | "quiet";
}> = ({ label, value, note, noteTone = "quiet" }) => (
  <div className="rounded-3xl bg-surface p-5">
    <div className="text-sm font-bold text-soft">{label}</div>
    <div className="text-3xl font-extrabold tracking-tight mt-1 tabular-nums">{value}</div>
    {note && (
      <div
        className={`text-sm mt-1 font-bold ${
          noteTone === "ok"
            ? "text-ok"
            : noteTone === "warn"
            ? "text-sun"
            : noteTone === "bad"
            ? "text-danger"
            : "text-faint"
        }`}
      >
        {note}
      </div>
    )}
  </div>
);

type LogFilter = "all" | "warn" | "error";

export const MonitorView: React.FC = () => {
  const { status, telemetry, telemetryHistory, logs, properties, sendCommand, clearLogs } =
    useServerStore();

  const live = status === "running" || status === "starting";
  const history = telemetryHistory;

  const tps = telemetry?.tps ?? 20;
  const ramUsed = telemetry?.ram_used_mb ?? 0;
  const ramMax = Math.max(1, telemetry?.ram_allocated_mb ?? 4096);
  const cpu = telemetry?.cpu_usage_percent ?? 0;
  const players = telemetry?.online_players.length ?? 0;

  const times = useMemo(() => history.map((p) => p.time), [history]);
  const tpsValues = useMemo(() => history.map((p) => p.tps), [history]);
  const ramValues = useMemo(() => history.map((p) => p.ramUsedMb), [history]);
  const cpuValues = useMemo(() => history.map((p) => p.cpuPercent), [history]);
  const cpuMax = Math.max(25, Math.ceil(Math.max(...cpuValues, 0) / 25) * 25);

  const tpsNote =
    tps >= 18 ? { t: "Running smoothly", tone: "ok" as const }
    : tps >= 14 ? { t: "A bit busy", tone: "warn" as const }
    : { t: "Lagging", tone: "bad" as const };
  const ramPct = (ramUsed / ramMax) * 100;
  const ramNote =
    ramPct >= 90 ? { t: "Almost full", tone: "bad" as const }
    : ramPct >= 75 ? { t: "Getting full", tone: "warn" as const }
    : { t: `${ramPct.toFixed(0)}% used`, tone: "quiet" as const };

  return (
    <Page
      title="Monitor"
      subtitle="How your world is doing right now, plus everything the server says."
    >
      <div className="flex items-center gap-2 -mt-3 text-sm font-bold text-soft">
        <StatusDot tone={status === "running" ? "on" : live || status === "provisioning" || status === "stopping" ? "busy" : "off"} />
        {status === "running"
          ? "Live"
          : status === "stopped"
          ? "World is closed — numbers appear once it's running"
          : "Getting ready…"}
      </div>

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <Tile
          label="Speed (TPS)"
          value={live ? tps.toFixed(1) : "—"}
          note={live ? tpsNote.t : undefined}
          noteTone={tpsNote.tone}
        />
        <Tile
          label="Memory"
          value={live ? gb(ramUsed) : "—"}
          note={live ? `${ramNote.t} · of ${gb(ramMax)}` : undefined}
          noteTone={ramNote.tone}
        />
        <Tile label="Processor" value={live ? `${cpu.toFixed(0)}%` : "—"} note={live ? "of your PC" : undefined} />
        <Tile
          label="Online for"
          value={live ? formatUptime(telemetry?.uptime_secs ?? 0) : "—"}
          note={live ? `${players} ${players === 1 ? "player" : "players"}${properties ? ` of ${properties.max_players}` : ""}` : undefined}
        />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <Card title="Speed" hint="Ticks per second. 20 is perfect.">
          <LiveChart
            values={tpsValues}
            times={times}
            max={20}
            format={(v) => v.toFixed(0)}
            colorClass="text-brand"
            label="Speed"
          />
        </Card>
        <Card title="Memory" hint="Used by the server.">
          <LiveChart
            values={ramValues}
            times={times}
            max={ramMax}
            format={gb}
            colorClass="text-info"
            label="Memory"
          />
        </Card>
        <Card title="Processor" hint="Share of your whole PC.">
          <LiveChart
            values={cpuValues}
            times={times}
            max={cpuMax}
            format={(v) => `${v.toFixed(0)}%`}
            colorClass="text-violet"
            label="Processor"
          />
        </Card>
      </div>

      <ConsolePanel
        logs={logs}
        enabled={status === "running"}
        onSend={sendCommand}
        onClear={clearLogs}
      />
    </Page>
  );
};

/* ---------- Console ---------- */

const ConsolePanel: React.FC<{
  logs: { timestamp: string; level: string; message: string }[];
  enabled: boolean;
  onSend: (cmd: string) => Promise<void>;
  onClear: () => void;
}> = ({ logs, enabled, onSend, onClear }) => {
  const [cmd, setCmd] = useState("");
  const [filter, setFilter] = useState<LogFilter>("all");
  const [history, setHistory] = useState<string[]>([]);
  const [historyIdx, setHistoryIdx] = useState<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);

  const visible = useMemo(() => {
    if (filter === "all") return logs;
    return logs.filter((l) => {
      const { level } = parseLogEntry(l);
      return filter === "error" ? level === "ERROR" : level === "WARN" || level === "ERROR";
    });
  }, [logs, filter]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickToBottom.current) el.scrollTop = el.scrollHeight;
  }, [visible]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    stickToBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const c = cmd.trim();
    if (!c) return;
    setHistory((h) => [...h.filter((x) => x !== c), c].slice(-50));
    setHistoryIdx(null);
    setCmd("");
    stickToBottom.current = true;
    await onSend(c);
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    if (history.length === 0) return;
    e.preventDefault();
    const next =
      e.key === "ArrowUp"
        ? Math.max(0, (historyIdx ?? history.length) - 1)
        : historyIdx === null
        ? null
        : historyIdx + 1 >= history.length
        ? null
        : historyIdx + 1;
    setHistoryIdx(next);
    setCmd(next === null ? "" : history[next]);
  };

  return (
    <Card
      title="Console"
      action={
        <div className="flex items-center gap-2">
          <Segmented<LogFilter>
            value={filter}
            onChange={setFilter}
            options={[
              { id: "all", label: "All" },
              { id: "warn", label: "Warnings" },
              { id: "error", label: "Errors" },
            ]}
          />
          <Button
            size="sm"
            variant="ghost"
            onClick={onClear}
            disabled={logs.length === 0}
            icon={<Trash2 className="w-4 h-4" />}
          >
            Clear
          </Button>
        </div>
      }
    >
      <div className="rounded-2xl bg-bg overflow-hidden">
        <div
          ref={scrollRef}
          onScroll={onScroll}
          className="h-[420px] overflow-y-auto p-4 font-mono text-[13px] leading-relaxed select-text"
        >
          {visible.length === 0 ? (
            <div className="text-faint">
              {logs.length === 0
                ? "Nothing here yet. Messages from the server appear once it starts."
                : "No lines match this filter."}
            </div>
          ) : (
            visible.map((l, i) => <LogLine key={i} entry={l} />)
          )}
        </div>
        <form className="flex gap-2 p-3 border-t border-line/60" onSubmit={submit}>
          <TextInput
            value={cmd}
            onChange={(e) => setCmd(e.target.value)}
            onKeyDown={onKeyDown}
            disabled={!enabled}
            placeholder={enabled ? "Type a command, like: say hello" : "Start your world to use commands"}
            className="font-mono text-sm"
            aria-label="Server command"
          />
          <Button type="submit" disabled={!enabled || !cmd.trim()} icon={<Send className="w-4 h-4" />}>
            Send
          </Button>
        </form>
      </div>
    </Card>
  );
};
