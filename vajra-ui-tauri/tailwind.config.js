/** @type {import('tailwindcss').Config} */
// Tailwind v4 is CSS-first. All design tokens live in src/index.css @theme block.
// This file only configures content scanning and any non-color extensions.
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        // Matches --font-sans in index.css
        sans: ['Inter', 'Segoe UI', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Consolas', 'monospace'],
      },
      animation: {
        'fade-in': 'fadeIn 0.18s ease',
        'slide-up': 'slideUp 0.18s ease',
        'slide-down': 'slideUp 0.18s ease reverse',
        'dialog-in': 'slideDialog 0.18s ease',
        'pulse-slow': 'pulse 2s ease infinite',
      },
    },
  },
  plugins: [],
};
