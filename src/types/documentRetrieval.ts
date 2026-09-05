export interface DocumentPassage {
  fileId: string
  jobId: string
  fileName: string
  sourcePath: string
  number: number
  locationKind: 'page' | 'segment'
  content: string
  citation: string
}
