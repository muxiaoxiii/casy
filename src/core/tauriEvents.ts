import { listen as tauriListen } from '@tauri-apps/api/event'
import { getCurrentScope, onScopeDispose } from 'vue'
import { isTauriRuntime } from './mockData'

export function safeListen<T = unknown>(event: string, handler: (event: { payload: T }) => void): (() => void) & { ready: Promise<void> } {
  let disposed = false, unlisten: (() => void) | undefined
  const stop = () => { disposed=true;unlisten?.();unlisten=undefined }
  if(getCurrentScope())onScopeDispose(stop)
  const ready = isTauriRuntime() ? tauriListen<T>(event, value => { if(!disposed)handler(value) }).then(off => {
      if(disposed)off();else unlisten=off
    }).catch(error => {console.warn(`[Casy] 事件监听未建立: ${event}`, error)}) : Promise.resolve()
  return Object.assign(stop,{ready})
}
