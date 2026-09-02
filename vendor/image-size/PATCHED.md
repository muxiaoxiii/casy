# Casy 本地补丁说明

本目录是 `image-size@1.2.1`（MIT，LICENSE 保留）的 vendored 副本，通过根 `package.json` 的
`overrides` 强制替换 `html-to-docx` 传递依赖中的上游版本。

## 补丁内容（GHSA-w3rx-r6r6-pgpr / GHSA-5p2g-fcmc-qvqq）

上游截至 2.0.2 未发布修复（OSV 记录 `last_affected: 2.0.2`），npm audit 的两项高危 DoS
无法通过升级解决，因此本地修补：

1. `dist/types/icns.js`：条目长度 `< 8`（含 0）时抛出异常，杜绝 `imageOffset`
   不前移造成的 `while` 死循环。
2. `dist/types/utils.js` 的 `findBox` 在本版本中已带零长度盒守卫（JXL/HEIF 链路），
   无需额外改动。

## 升级策略

上游发布修复版本后，删除根 `package.json` 的 override 与本目录，回到官方包。

回归测试：`tests/image-size-dos.test.ts`（恶意 ICNS 必须在毫秒级返回而非挂起）。
