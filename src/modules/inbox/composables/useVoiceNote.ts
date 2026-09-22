/**
 * 语音速记 composable（设计哲学 §10）
 * 按住说话 / 录音转写 → 收件箱
 */
import { ref, onScopeDispose } from 'vue'
import { ElMessage } from 'element-plus'
import { tauriCallSafe } from '../../../core/tauriBridge'

export function useVoiceNote() {
  const isRecording = ref(false)
  const isSaving = ref(false)
  let disposed=false
  const recordingTime = ref(0)
  const transcript = ref('')
  let mediaRecorder: MediaRecorder | null = null
  let stream: MediaStream | null = null
  let audioChunks: Blob[] = []
  let timer: number | null = null

  /** 开始录音 */
  async function startRecording() {
    if(disposed || isRecording.value || isSaving.value)return
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true })
      if(disposed){stream.getTracks().forEach(t=>t.stop());stream=null;return}
      mediaRecorder = new MediaRecorder(stream)
      audioChunks = []

      mediaRecorder.ondataavailable = (event) => {
        audioChunks.push(event.data)
      }

      mediaRecorder.onstop = async () => {
        isSaving.value=true
        const audioBlob = new Blob(audioChunks, { type: mediaRecorder?.mimeType || 'audio/webm' })
        stream?.getTracks().forEach(track => track.stop())
        stream = null
        try { await processAudio(audioBlob);audioChunks=[] } catch(error) {
          ElMessage.error(`录音保存失败：${String(error)}。已尝试下载音频副本，请保留原件。`)
          const url=URL.createObjectURL(audioBlob),a=document.createElement('a');a.href=url;a.download='未保存的语音速记.'+(audioBlob.type.includes('mp4')?'m4a':audioBlob.type.includes('ogg')?'ogg':'webm');a.click();setTimeout(()=>URL.revokeObjectURL(url),60000)
        } finally {isSaving.value=false}
      }

      mediaRecorder.start()
      isRecording.value = true
      recordingTime.value = 0

      // 计时器
      timer = window.setInterval(() => {
        recordingTime.value++
        if(recordingTime.value>=3600)stopRecording()
      }, 1000)

    } catch (err) {
      console.error('录音失败:', err)
      stream?.getTracks().forEach(track => track.stop())
      stream = null
      throw err
    }
  }

  /** 停止录音 */
  function stopRecording() {
    if (mediaRecorder && isRecording.value) {
      mediaRecorder.stop()
      isRecording.value = false
      if (timer) {
        clearInterval(timer)
        timer = null
      }
    }
  }

  // 组件卸载兜底：清计时器、停录音（onstop 负责保存并释放麦克风），
  // 防止录音中离开页面导致麦克风常亮
  onScopeDispose(() => {
    disposed=true
    if (timer) {
      clearInterval(timer)
      timer = null
    }
    if (mediaRecorder && mediaRecorder.state !== 'inactive') {
      mediaRecorder.stop()
    } else {
      stream?.getTracks().forEach(track => track.stop())
      stream = null
    }
    isRecording.value = false
  })

  /** 处理音频 → 转写 → 添加到收件箱 */
  async function processAudio(audioBlob: Blob) {
    const audioBase64=await new Promise<string>((resolve,reject)=>{
      const reader=new FileReader();reader.onerror=()=>reject(reader.error);reader.onload=()=>resolve(String(reader.result).split(',')[1] || '');reader.readAsDataURL(audioBlob)
    })
    const result=await tauriCallSafe('save_voice_note',{audioBase64,mimeType:audioBlob.type,durationSeconds:recordingTime.value})
    if(!result.ok)throw new Error(result.error || '录音保存失败')
    transcript.value='';ElMessage.success('录音原件已存入收件箱，尚未转写')
  }

  /** 格式化录音时长 */
  function formatTime(seconds: number): string {
    const m = Math.floor(seconds / 60)
    const s = seconds % 60
    return `${m}:${String(s).padStart(2, '0')}`
  }

  return {
    isRecording,
    isSaving,
    recordingTime,
    transcript,
    startRecording,
    stopRecording,
    formatTime,
  }
}
