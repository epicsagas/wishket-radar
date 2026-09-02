import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { bootstrapToken } from './api'
import { applyTheme, readTheme } from './lib/theme'

applyTheme(readTheme())
bootstrapToken()

const app = mount(App, { target: document.getElementById('app')! })

export default app
