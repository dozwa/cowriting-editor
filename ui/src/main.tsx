import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { api } from './api'
import { App } from './App'
import './styles.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App api={api} />
  </StrictMode>,
)
