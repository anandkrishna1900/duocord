# Working on this repo

## UI work
For any UI change (desktop React or Android Compose), take components and inspiration from these and use the design skills
(ui-ux-pro-max and related): React Bits (github.com/DavidHDev/react-bits; MIT + Commons Clause), 21st.dev/community/components,
uiverse.io, mobbin.com/discover/apps/ios/latest, magicui.design, ui.watermelon.sh, untitledui.com/react/components, jitter.video/templates/ui-elements, reactbits.dev/c/components. Some of them refuse automated fetches (403): use what loads, or
ask for pasted content. The desktop has no Tailwind, so effects are re-implemented in plain CSS (see
`apps/desktop/src/polish.css`); keep them cheap and honour `prefers-reduced-motion`. Check licences before copying code.
