export interface DocumentPassage {
  fileId: string
  jobId: string
  fileName: string
  sourcePath: string
  number: number
  locationKind: 'page' | 'segment'
  content: string
  citation: string
  locations: SourceLocation[]
}

export interface SourceLocation {
  pageNumber: number
  regionIndex: number | null
  bbox: [number, number, number, number] | null
  width: number | null
  height: number | null
}

export interface DocumentPageView {
  fileId: string
  jobId: string
  fileName: string
  pageNumber: number
  totalPages: number
  markdown: string
  imageData: string | null
  width: number | null
  height: number | null
  regions: Array<{text:string; bbox:[number,number,number,number]; confidence:number|null}>
}
