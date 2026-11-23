import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { type Api, api as tauriApi } from './api'
import { App } from './App'
import './styles.css'

// in the app the commands go to Tauri; in a plain browser during development, to the bridge
const inApp = '__TAURI_INTERNALS__' in window
const api: Api = inApp || !import.meta.env.DEV ? tauriApi : (await import('./bridge')).bridgeApi

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App api={api} />
  </StrictMode>,
)
