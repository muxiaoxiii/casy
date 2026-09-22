function layoutGraph(nodeList, edgeList, width, height) {
  const n = nodeList.length
  if (!n) return
  const idx = new Map(nodeList.map((nd, i) => [nd.id, i]))

  // 初始：均匀环形
  const r0 = Math.min(width, height) * 0.35
  nodeList.forEach((nd, i) => {
    const a = (i / n) * Math.PI * 2
    nd.x = width / 2 + Math.cos(a) * r0
    nd.y = height / 2 + Math.sin(a) * r0
  })

  const ITER = 160
  for (let it = 0; it < ITER; it++) {
    const cooling = 1 - it / ITER

    // 斥力（全对）
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        let dx = nodeList[i].x - nodeList[j].x
        let dy = nodeList[i].y - nodeList[j].y
        let d2 = dx * dx + dy * dy
        if (d2 < 1) { dx = Math.random() - 0.5; dy = Math.random() - 0.5; d2 = 1 }
        const d = Math.sqrt(d2)
        const f = Math.min(2400 / d2, 12) * cooling
        const fx = (dx / d) * f
        const fy = (dy / d) * f
        nodeList[i].x += fx; nodeList[i].y += fy
        nodeList[j].x -= fx; nodeList[j].y -= fy
      }
    }

    // 边弹簧（目标距离 120）
    for (const e of edgeList) {
      const a = nodeList[idx.get(e.source)]
      const b = nodeList[idx.get(e.target)]
      if (!a || !b) continue
      const dx = b.x - a.x
      const dy = b.y - a.y
      const d = Math.max(1, Math.hypot(dx, dy))
      const f = (d - 120) * 0.02 * cooling
      const fx = (dx / d) * f
      const fy = (dy / d) * f
      a.x += fx; a.y += fy
      b.x -= fx; b.y -= fy
    }

    // 向心重力
    for (const nd of nodeList) {
      nd.x += (width / 2 - nd.x) * 0.01
      nd.y += (height / 2 - nd.y) * 0.01
    }
  }

  // 收进画布
  for (const nd of nodeList) {
    nd.x = Math.max(50, Math.min(width - 50, nd.x))
    nd.y = Math.max(40, Math.min(height - 40, nd.y))
  }
}

self.onmessage = ({data}) => {layoutGraph(data.nodes,data.edges,data.width,data.height);self.postMessage(data.nodes)}
