import './assets/styles/globals.css'
import 'katex/dist/katex.min.css'
import 'streamdown/styles.css'

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { watchSystemTheme } from './lib/systemTheme'

const desktopPlatform = window.electron?.process.platform
document.documentElement.dataset.desktopPlatform = desktopPlatform ?? 'unknown'
if (desktopPlatform === 'darwin') {
  document.documentElement.dataset.nativeBackdrop = 'true'
}

watchSystemTheme()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>
)
