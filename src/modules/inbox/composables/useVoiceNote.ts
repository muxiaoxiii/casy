/**
 * 语音速记 composable（设计哲学 §10）
 * 按住说话 / 录音转写 → 收件箱
 */
import { ref, onScopeDispose } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'

export function useVoiceNote() {
  const isRecording = ref(false)
  const recordingTime = ref(0)
  const transcript = ref('')
  let mediaRecorder: MediaRecorder | null = null
  let stream: MediaStream | null = null
  let audioChunks: Blob[] = []
  let timer: number | null = null

  /** 开始录音 */
  async function startRecording() {
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true })
      mediaRecorder = new MediaRecorder(stream)
      audioChunks = []

      mediaRecorder.ondataavailable = (event) => {
        audioChunks.push(event.data)
      }

      mediaRecorder.onstop = async () => {
        const audioBlob = new Blob(audioChunks, { type: 'audio/webm' })
        await processAudio(audioBlob)
        stream?.getTracks().forEach(track => track.stop())
        stream = null
      }

      mediaRecorder.start()
      isRecording.value = true
      recordingTime.value = 0

      // 计时器
      timer = window.setInterval(() => {
        recordingTime.value++
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
    // 将音频转换为 base64
    const reader = new FileReader()
    reader.onloadend = async () => {
      const base64 = (reader.result as string).split(',')[1]

      // 保存到临时文件
      const result = await tauriCallSafe('save_voice_note', {
        audioData: base64,
        mimeType: 'audio/webm',
      })

      if (result.ok && result.data) {
        // 添加到收件箱
        await tauriCallSafe('add_inbox_item', {
          sourceType: 'voice',
          title: `语音速记 ${new Date().toLocaleTimeString()}`,
          contentText: transcript.value || '（语音待转写）',
          sourcePath: result.data.path,
        })

        transcript.value = ''
      }
    }
    reader.readAsDataURL(audioBlob)
  }

  /** 格式化录音时长 */
  function formatTime(seconds: number): string {
    const m = Math.floor(seconds / 60)
    const s = seconds % 60
    return `${m}:${String(s).padStart(2, '0')}`
  }

  return {
    isRecording,
    recordingTime,
    transcript,
    startRecording,
    stopRecording,
    formatTime,
  }
}
