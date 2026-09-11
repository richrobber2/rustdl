"""Run an installed development APK and stream coverage/results to the terminal."""
import json, subprocess, time, urllib.request, uuid, hashlib, os, sys
from pathlib import Path
from visual_report import write_report
root=Path(__file__).resolve().parents[2]
batch500='--batch500' in sys.argv
manifest=root/'target/android-termux-alongside-dev/apk/assets/dev/suite.json'
if not manifest.exists():raise SystemExit('Build and install make dev-apk first')
coverage=json.loads(manifest.read_text())
if batch500:
 coverage['cases']=[case for case in coverage['cases'] if '-500/' in case['name']]
 coverage['missing']=[]
 if len(coverage['cases'])!=6:raise SystemExit('Rebuild dev APK with the six populated 500-item cases')

expected=coverage['sourceHashes']
stale=[]
current=[p for pattern in ['assets/html/*.html','assets/css/*.css','assets/js/*.js','android/*.java','android/AndroidManifest.xml','src/dev/android/*.java','src/dev/visual-*.js','src/dev/*.py','android/build-termux.sh','assets/images/**/*.webp'] for p in root.glob(pattern)]
for path in current:
 target=str(path.relative_to(root))
 if target not in expected or hashlib.sha256(path.read_bytes()).hexdigest()!=expected[target]:
  stale.append(target);print('MISSING '+target+': new or changed since development APK build',flush=True)
if stale:raise SystemExit('Rebuild and install make dev-apk before testing changed UI sources')
apk=root/'target/android-termux-alongside-dev/rustdl.apk'
installed=subprocess.check_output(['/system/bin/pm','path','app.rustdl.next'],text=True).strip().removeprefix('package:')
if not installed or hashlib.sha256(Path(installed).read_bytes()).digest()!=hashlib.sha256(apk.read_bytes()).digest():
 raise SystemExit('MISSING current development APK installation; install make dev-apk output first')

for row in coverage['missing']:print('MISSING '+row['target']+': '+row['reason'],flush=True)
node_env=os.environ.copy()
if 'NODE_PATH' not in node_env:
 for dependencies in [root/'tests/ui/node_modules',root/'target/ui-tests/node_modules']:
  if (dependencies/'jsdom').is_dir():node_env['NODE_PATH']=str(dependencies);break
subprocess.run(['node','--test',str(root/'src/dev/visual-audit.test.cjs'),str(root/'src/dev/visual-smoke.test.cjs')],env=node_env,check=True)
run_id=uuid.uuid4().hex
subprocess.run(['am','start','-n','app.rustdl.next/app.rustdl.GalleryBenchmarkActivity','--ez','auto','true','--ez','suite','true','--es','run_id',run_id]+(['--ez','batch500','true'] if batch500 else []),check=True)
seen=set();started=time.monotonic()
while time.monotonic()-started<max(240,len(coverage['cases'])*4+90):
 try:
  with urllib.request.urlopen('http://127.0.0.1:39090/report.json',timeout=3) as r:report=json.load(r)
  if report.get('runId')!=run_id:
   time.sleep(2);continue
  for case in report.get('visualCases',[]):
   if case['name'] in seen:continue
   seen.add(case['name'])
   screenshot=case.get('screenshot','')
   if screenshot.startswith('/failure-') and screenshot.endswith('.png') and '/' not in screenshot[1:]:
    try:
     dest=root/'target/gallery-performance/visual-failures'/run_id/screenshot[1:];dest.parent.mkdir(parents=True,exist_ok=True)
     with urllib.request.urlopen('http://127.0.0.1:39090'+screenshot,timeout=5) as response:dest.write_bytes(response.read())
     case['savedScreenshot']=str(dest.relative_to(root))
    except OSError as error:print('MISSING screenshot: '+str(error),flush=True)
   print(('PASS ' if case.get('pass') else 'FAIL ')+case['name']+' '+json.dumps(case),flush=True)
  if report.get('state')=='complete':
   out=root/('target/gallery-performance/batch500-visual-report.json' if batch500 else 'target/gallery-performance/visual-suite-report.json');out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(report,indent=2))
   write_report(report,out.with_suffix('.html'),root/'target/gallery-performance/visual-failures'/run_id)
   for phase in report.get('results',[]):print('SCROLL '+json.dumps(phase),flush=True)
   print(f'COMPLETE {len(seen)}/{len(coverage["cases"])} visual cases; {len(coverage["missing"])} coverage gaps; report: {out}',flush=True)
   raise SystemExit(1 if report.get('aborted') or len(seen)!=len(coverage['cases']) or any(not c.get('pass') for c in report.get('visualCases',[])) else 0)
 except (OSError,ValueError) as error:print('WAIT '+type(error).__name__,flush=True)
 time.sleep(2)
raise SystemExit('FAIL Visual suite timed out; keep the development test visible and unlocked')
