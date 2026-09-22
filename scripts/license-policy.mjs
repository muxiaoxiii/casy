// Conservative allow-list: ambiguous, custom and copyleft licenses keep their archives.
const noticeOnly = new Set(['MIT','Apache-2.0','ISC','BSD-2-Clause','BSD-3-Clause','BSD-3-Clause-Clear','0BSD','Zlib','Unicode-3.0','Unicode-DFS-2016','Unlicense','CC0-1.0','BSL-1.0','OFL-1.1','BlueOak-1.0.0'])
export function sourceArchiveRequired(license) {
  if (typeof license !== 'string' || !license.trim()) return true
  const tokens=license.replaceAll('/', ' OR ').match(/\(|\)|[^\s()]+/g) || []
  let cursor=0
  function expression() {
    function atom() {
      if(tokens[cursor]==='('){
        cursor++
        if(!expression() || tokens[cursor++]!==')')return false
        return true
      }
      return noticeOnly.has(tokens[cursor++])
    }
    if(!atom())return false
    while(tokens[cursor]==='AND'||tokens[cursor]==='OR'){
      cursor++
      if(!atom())return false
    }
    return true
  }
  return !expression() || cursor!==tokens.length
}
