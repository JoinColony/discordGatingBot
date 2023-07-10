/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./www/*.html', './src/*.{ts,tsx}'],
  theme: {
    colors: {
      blue: {
        1000: '#021C2B',
        900: '#012138',
        800: '#00284B',
        400: '#289BDC',
      },
      green: '#19A582',
      grey: {
        500: '#A4B6C7',
      },
      pink: '#F5416E',
      purple: {
        1000: '#101828',
        500: '#5865F2',
      },
      transparent: 'transparent',
      white: '#ffffff',
    },
    fontFamily: {
      sans: ['Avenir Next', 'sans-serif'],
      mono: ['IBM Plex Mono', 'monospace'],
    },
  },
  plugins: [],
}

