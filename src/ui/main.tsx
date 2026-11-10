import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App.js'
import { LangRoot } from './i18n.js'
import './styles.css'

const el = document.getElementById('root')
if (!el) throw new Error('нет #root')
createRoot(el).render(
  <StrictMode>
    <LangRoot>
      <App />
    </LangRoot>
  </StrictMode>,
)
