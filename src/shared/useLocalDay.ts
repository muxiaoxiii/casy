import { ref, readonly, onScopeDispose } from 'vue'
const day = ref('')
let users = 0, timer: ReturnType<typeof setInterval> | undefined
function update() { const d=new Date(); day.value=`${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}` }
export function useLocalDay() {
  if(users++===0) { update();timer=setInterval(update,30000);window.addEventListener('focus',update);document.addEventListener('visibilitychange',update) }
  onScopeDispose(()=>{if(--users===0){clearInterval(timer);window.removeEventListener('focus',update);document.removeEventListener('visibilitychange',update)}})
  return readonly(day)
}
