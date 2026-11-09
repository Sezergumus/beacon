import React from "react";
import type { ServerLogEntry } from "./tauri";

export type LogLevel = "INFO" | "WARN" | "ERROR" | "DEBUG";

// eslint-disable-next-line no-control-regex
const ANSI = /\u001b\[[0-9;?]*[A-Za-z]/g;
const SECTION_CODES = /§[0-9a-fk-orx]/gi;

// Paper:  [12:34:56 INFO]: text
const HEADER_PAPER = /^\[(\d{2}:\d{2}:\d{2})(?:\.\d+)? (INFO|WARN|WARNING|ERROR|FATAL|DEBUG)\]:?\s?/;
// Vanilla / Fabric:  [12:34:56] [Server thread/INFO]: text
const HEADER_VANILLA =
  /^\[(\d{2}:\d{2}:\d{2})(?:\.\d+)?\] \[[^\]/]+\/(INFO|WARN|WARNING|ERROR|FATAL|DEBUG)\](?: \[[^\]]*\])?:?\s?/;

const normalizeLevel = (raw: string): LogLevel => {
  switch (raw.toUpperCase()) {
    case "WARN":
    case "WARNING":
      return "WARN";
    case "ERROR":
    case "FATAL":
      return "ERROR";
    case "DEBUG":
      return "DEBUG";
    default:
      return "INFO";
  }
};

export interface ParsedLog {
  time: string;
  level: LogLevel;
  body: string;
}

export function parseLogEntry(entry: ServerLogEntry): ParsedLog {
  const clean = entry.message.replace(ANSI, "").replace(SECTION_CODES, "");
  const m = HEADER_PAPER.exec(clean) ?? HEADER_VANILLA.exec(clean);
  if (m) {
    return { time: m[1], level: normalizeLevel(m[2]), body: clean.slice(m[0].length) };
  }
  return { time: entry.timestamp, level: normalizeLevel(entry.level), body: clean };
}

/* ---------- Body highlighting (INFO lines only) ---------- */

const TOKENS: { cls: string; src: string }[] = [
  // names next to join / leave messages
  { cls: "text-brand font-bold", src: "[A-Za-z0-9_]{3,16}(?= (?:joined|left) the game)" },
  { cls: "text-brand font-bold", src: "(?<=UUID of player )[A-Za-z0-9_]{3,16}" },
  // good news
  { cls: "text-ok font-bold", src: "Done \\([\\d.,]+s\\)!|joined the game" },
  // trouble
  {
    cls: "text-danger",
    src: "[\\w.$]*(?:Exception|Error)\\b|Can't keep up!|\\b[Ff]ailed\\b|\\bFAILED\\b|\\bCaused by\\b",
  },
  { cls: "text-sun", src: "left the game|Is the server overloaded\\?" },
  // chat names and [Tags]
  { cls: "text-violet", src: "<\\w{1,16}>|\\[[A-Za-z][\\w .\\-]*\\]" },
  // addresses, paths, files, numbers with units
  {
    cls: "text-info",
    src:
      "/?\\b\\d{1,3}(?:\\.\\d{1,3}){3}(?::\\d+)?\\b" +
      "|\\b[A-Za-z]:\\\\[^\\s\"']+" +
      "|\\b[\\w.\\-]+\\.(?:jar|json|yml|yaml|properties|dat|toml|zst)\\b" +
      "|\\b\\d+(?:[.,]\\d+)?\\s?(?:ms|MB|GB|KB|%|ticks?)(?![A-Za-z])" +
      "|\\b\\d+(?:[.,]\\d+)?s\\b",
  },
];

const TOKEN_RE = new RegExp(TOKENS.map((t) => `(${t.src})`).join("|"), "g");

function highlight(body: string): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  let last = 0;
  let key = 0;
  for (const m of body.matchAll(TOKEN_RE)) {
    const start = m.index ?? 0;
    if (start > last) out.push(body.slice(last, start));
    const group = m.findIndex((g, i) => i > 0 && g !== undefined) - 1;
    out.push(
      <span key={key++} className={TOKENS[group]?.cls}>
        {m[0]}
      </span>
    );
    last = start + m[0].length;
  }
  if (last < body.length) out.push(body.slice(last));
  return out;
}

export const LEVEL_BADGE: Record<LogLevel, string> = {
  INFO: "text-info bg-info/15",
  WARN: "text-sun bg-sun/15",
  ERROR: "text-danger bg-danger/15",
  DEBUG: "text-faint bg-line/50",
};

/** Renders one console line with colors: level badge, dim time, highlighted body. */
export const LogLine: React.FC<{ entry: ServerLogEntry }> = React.memo(({ entry }) => {
  const { time, level, body } = parseLogEntry(entry);

  let content: React.ReactNode;
  let bodyClass = "text-ink";
  if (level === "WARN") {
    bodyClass = "text-sun";
    content = body;
  } else if (level === "ERROR") {
    bodyClass = "text-danger";
    content = body;
  } else if (level === "DEBUG") {
    bodyClass = "text-faint";
    content = body;
  } else if (/^\s+at\s/.test(body) || /^\s*\.\.\. \d+ more/.test(body)) {
    // stack trace frames
    bodyClass = "text-faint";
    content = body;
  } else if (/^Caused by/.test(body)) {
    bodyClass = "text-danger";
    content = body;
  } else {
    content = highlight(body);
  }

  return (
    <div className="flex gap-2.5 py-px">
      <span className="text-faint shrink-0">{time}</span>
      <span
        className={`shrink-0 w-12 text-center text-[11px] font-bold h-[18px] leading-[18px] mt-[2px] rounded-md ${LEVEL_BADGE[level]}`}
      >
        {level}
      </span>
      <span className={`min-w-0 whitespace-pre-wrap break-words ${bodyClass}`}>{content}</span>
    </div>
  );
});
LogLine.displayName = "LogLine";
