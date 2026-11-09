/** Minecraft colour codes (`§0`-`§f`) and the colours they stand for. */
export const MOTD_COLORS: { code: string; name: string; hex: string }[] = [
  { code: "0", name: "Black", hex: "#000000" },
  { code: "1", name: "Dark blue", hex: "#0000AA" },
  { code: "2", name: "Dark green", hex: "#00AA00" },
  { code: "3", name: "Dark aqua", hex: "#00AAAA" },
  { code: "4", name: "Dark red", hex: "#AA0000" },
  { code: "5", name: "Purple", hex: "#AA00AA" },
  { code: "6", name: "Gold", hex: "#FFAA00" },
  { code: "7", name: "Gray", hex: "#AAAAAA" },
  { code: "8", name: "Dark gray", hex: "#555555" },
  { code: "9", name: "Blue", hex: "#5555FF" },
  { code: "a", name: "Green", hex: "#55FF55" },
  { code: "b", name: "Aqua", hex: "#55FFFF" },
  { code: "c", name: "Red", hex: "#FF5555" },
  { code: "d", name: "Pink", hex: "#FF55FF" },
  { code: "e", name: "Yellow", hex: "#FFFF55" },
  { code: "f", name: "White", hex: "#FFFFFF" },
];

export const MOTD_FORMATS: { code: string; name: string; label: string }[] = [
  { code: "l", name: "Bold", label: "B" },
  { code: "o", name: "Italic", label: "I" },
  { code: "n", name: "Underline", label: "U" },
  { code: "m", name: "Strikethrough", label: "S" },
  { code: "k", name: "Scrambled", label: "?" },
];

export const MOTD_RESET = "r";

export interface MotdSpan {
  text: string;
  color: string | null;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  obfuscated: boolean;
}

const COLOR_BY_CODE = new Map(MOTD_COLORS.map((c) => [c.code, c.hex]));

/**
 * Split a welcome message into lines of styled pieces the way Minecraft reads it:
 * `§` plus a code changes the style, a colour code also clears the other formats,
 * and `§r` clears everything.
 */
export const parseMotd = (text: string): MotdSpan[][] =>
  text.split("\n").map((line) => {
    const spans: MotdSpan[] = [];
    let style = { color: null as string | null, bold: false, italic: false, underline: false, strike: false, obfuscated: false };
    let buffer = "";
    const flush = () => {
      if (buffer) spans.push({ text: buffer, ...style });
      buffer = "";
    };
    for (let i = 0; i < line.length; i++) {
      const ch = line[i];
      if (ch === "§" && i + 1 < line.length) {
        const code = line[i + 1].toLowerCase();
        i++;
        flush();
        if (COLOR_BY_CODE.has(code)) {
          style = { color: COLOR_BY_CODE.get(code)!, bold: false, italic: false, underline: false, strike: false, obfuscated: false };
        } else if (code === "l") style = { ...style, bold: true };
        else if (code === "o") style = { ...style, italic: true };
        else if (code === "n") style = { ...style, underline: true };
        else if (code === "m") style = { ...style, strike: true };
        else if (code === "k") style = { ...style, obfuscated: true };
        else if (code === MOTD_RESET) {
          style = { color: null, bold: false, italic: false, underline: false, strike: false, obfuscated: false };
        }
        continue;
      }
      buffer += ch;
    }
    flush();
    return spans;
  });
