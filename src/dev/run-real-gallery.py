"""Measure the actual installed gallery. Read only sanitized numeric reports."""
from pathlib import Path
import hashlib,json,subprocess,time,urllib.request,uuid
root=Path(__file__).resolve().parents[2]
apk=root/'target/android-termux-alongside-dev/rustdl.apk'
installed=subprocess.check_output(['/system/bin/pm','path','app.rustdl.next'],text=True).strip().removeprefix('package:')
if not installed or hashlib.sha256(Path(installed).read_bytes()).digest()!=hashlib.sha256(apk.read_bytes()).digest():
 raise SystemExit('Install the current development APK before measuring the real gallery')
run_id=uuid.uuid4().hex
subprocess.run(['am','start','-n','app.rustdl.next/app.rustdl.MainActivity','--ez','dev_metrics','true','--es','metrics_run_id',run_id],check=True)
seen=set();names=['initial scroll, images visible','repeat scroll, images visible','repeat scroll, image painting hidden']
start=time.monotonic()
while time.monotonic()-start<120:
 try:
  with urllib.request.urlopen('http://127.0.0.1:39091/report.json',timeout=3) as r:report=json.load(r)
  if report.get('runId')!=run_id:time.sleep(2);continue
  for phase in report.get('phases',[]):
   index=int(phase['phase'])
   if index in seen:continue
   seen.add(index);print('METRICS '+names[index]+' '+json.dumps(phase),flush=True)
  if report.get('state')=='complete':
   destination=root/'target/gallery-performance/real-gallery-metrics.json';destination.parent.mkdir(parents=True,exist_ok=True);destination.write_text(json.dumps(report,indent=2))
   print('COMPLETE numeric metrics only; no media content or screenshots collected',flush=True)
   raise SystemExit(1 if report.get('aborted') or seen!={0,1,2} or any(p.get('aborted') for p in report['phases']) else 0)
 except (OSError,ValueError) as error:print('WAIT '+type(error).__name__,flush=True)
 time.sleep(2)
raise SystemExit('Metrics timed out; keep the actual gallery visible and unlocked')
