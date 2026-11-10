/** @type {import('tailwindcss').Config} */
const token = (name) => `rgb(var(--${name}) / <alpha-value>)`;

export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "Plus Jakarta Sans Variable",
          "Plus Jakarta Sans",
          "Segoe UI",
          "system-ui",
          "sans-serif",
        ],
        mono: ["Cascadia Code", "Consolas", "ui-monospace", "monospace"],
      },
      colors: {
        bg: token("bg"),
        surface: token("surface"),
        raised: token("raised"),
        line: token("line"),
        ink: token("ink"),
        soft: token("soft"),
        faint: token("faint"),
        brand: token("brand"),
        "brand-ink": token("brand-ink"),
        sun: token("sun"),
        danger: token("danger"),
        ok: token("ok"),
        info: token("info"),
        violet: token("violet"),
      },
      borderRadius: {
        xl: "14px",
        "2xl": "18px",
        "3xl": "24px",
      },
    },
  },
  plugins: [],
};
