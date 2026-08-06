/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Dark theme
        "dark-bg": "#0F172A",
        "dark-surface": "#1E293B",
        "dark-border": "#334155",
        // Light theme
        "light-bg": "#F8FAFC",
        "light-surface": "#FFFFFF",
        "light-border": "#E2E8F0",
        // Accent
        accent: {
          DEFAULT: "#22C55E",
          dark: "#16A34A",
        },
      },
      fontFamily: {
        mono: ["JetBrains Mono", "monospace"],
        sans: ["IBM Plex Sans", "system-ui", "sans-serif"],
      },
      transitionDuration: {
        DEFAULT: "200ms",
      },
    },
  },
  plugins: [require("@tailwindcss/typography")],
};
