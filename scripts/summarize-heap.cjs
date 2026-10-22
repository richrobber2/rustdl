// Text-only DHAT summaries. Input should come from our synthetic workloads.
const fs = require('node:fs');
const files = process.argv.slice(2);
if (!files.length) throw new Error('Usage: node scripts/summarize-heap.cjs target/heap-profiles/CASE-PID.json');
for (const file of files) {
  const data = JSON.parse(fs.readFileSync(file, 'utf8'));
  if (data.mode !== 'rust-heap' || !Array.isArray(data.pps) || !Array.isArray(data.ftbl)) {
    throw new Error(`${file}: unsupported DHAT heap format`);
  }
  const points = data.pps;
  const sum = key => points.reduce((n, p) => n + (p[key] || 0), 0);
  console.log(JSON.stringify({file, totalBytes: sum('tb'), totalBlocks: sum('tbk'), peakBytes: sum('gb'), endBytes: sum('eb')}));
  const frame = p => p.fs.map(i => data.ftbl[i]).find(f => /(?:heap_workload|rustdl::|heap_profile::app::)/.test(f)) || p.fs.map(i => data.ftbl[i]).find(f => f && !f.includes('dhat::')) || '(unknown)';
  // Allocation sites are exclusive: summing them does not count callers twice.
  for (const p of [...points].sort((a, b) => b.tb - a.tb).slice(0, 8)) {
    console.log(JSON.stringify({totalBytes: p.tb, totalBlocks: p.tbk, peakBytes: p.gb, endBytes: p.eb, frame: frame(p)}));
  }
}
