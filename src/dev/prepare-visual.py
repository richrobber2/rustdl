"""Discover UI templates and generate isolated visual-smoke fixtures for dev APKs."""
from pathlib import Path
import hashlib, json, re, sys
root=Path(__file__).resolve().parents[2]
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
styles={'playlist-resolution':['discovery'],'index':['index'],'gallery':['index'],'gallery-inspection':['index'],'streaming-card':['streaming-catalog'],'watchlist-card':['streaming-library'],'watchlist':['streaming-library'],'calendar':['streaming-library'],'inspection-player':['player'],'download-result':['player'],'playlist':['discovery','playlist'],'quality':['discovery','quality'],'storage-confirm':['storage']}
fragments={'gallery','gallery-inspection','streaming-card','watchlist-card'}
empty={'css','script','page_script','playback_script','view_transition_script','dev_reload','BULK_QUALITY_SCRIPT','PEER_PAIRING_SCRIPT','page_css','player_css','errors','notice','partials','hidden_file','new_status_suffix','saved_class','growing','paired','options','releases','stale_action','watched_action','thumbnail_action','day_nav','sections','collection_nav','initial_cards','badges_html','poster_html','tabs','previous_link','next_link'}
values={k:'' for k in empty}
values.update({'gallery_json':'[]','library_heading':'Synthetic gallery','library_summary':'0 synthetic items','count':'3','title_text':'Synthetic catalog','detail':'Synthetic development fixture. No personal media or external actions.','description':'Synthetic visual fixture','heading':'Development preview','status':'Ready','filename':'1-1.mp4','display_filename':'1-1.mp4','form_filename':'1-1.mp4','playlist_title':'Synthetic playlist','title':'Synthetic player','saved_path':'Synthetic Downloads','media_element':'<div class="synthetic-video" style="aspect-ratio:16/9;background:linear-gradient(135deg,#17434c,#543060);display:grid;place-items:center">Synthetic media surface</div>','qr':'<div style="width:180px;height:180px;background:repeating-conic-gradient(#fff 0% 25%,#111 0% 50%) 0/24px 24px" aria-label="Synthetic pairing pattern"></div>','address':'127.0.0.1:39090','receiver_address':'127.0.0.1:39090','key':'SYNTHETIC-NOT-A-PAIRING-KEY','token':'synthetic','action_token_value':'synthetic','poster_src':'/thumbnail/1-1.mp4.jpg','watch_url':'#','launch_href':'#','watch_href':'#','retry':'#','refresh':'#','return_path':'#','watchlist_action':'#','media_route':'#','search_value':'','section_value':'newest','app_version':'development','visit':'synthetic','codec':'MP4','action_icon':'+','action_label':'Save synthetic item','action_title':'Save synthetic item','transition_name':'synthetic-item'})
cases=[];missing=[]
(out/'visual-canary.html').write_text('''<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><style>
body{margin:16px;font:16px sans-serif}#clip{width:100px;height:24px;overflow:hidden}#clipped-button{height:48px}
#text{width:100px;white-space:nowrap;overflow:hidden}#ellipsis{width:100px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#overlap{position:relative;width:180px;height:50px}#covered{width:180px;height:50px}#cover{position:absolute;inset:0;background:#678}
#strip{width:150px;overflow-x:auto;white-space:nowrap}#spacer{display:inline-block;width:200px}#reachable{width:80px}
</style></head><body><div id="clip"><button id="clipped-button">Clipped</button></div><p id="text">Long text that is deliberately clipped without an ellipsis</p><p id="ellipsis">Intentional ellipsis remains a warning</p><div id="overlap"><button id="covered">Covered</button><div id="cover"></div></div><div id="strip"><span id="spacer"></span><button id="reachable">Reachable</button></div></body></html>''')
cases.append({'name':'detector-canary/native','asset':'visual-canary.html','profile':'native','script':'visual-canary.js','coverage':'Deliberate layout defects verify detection in the real WebView; not an app page'})

for template in sorted((root/'assets/html').glob('*.html')):
 name=template.stem
 cssnames=styles.get(name,['peers' if name.startswith('transfer-') else name])
 css=''
 for cssname in cssnames:
  p=root/'assets/css'/f'{cssname}.css'
  if p.exists():css+=p.read_text()
  else:missing.append({'target':str(p.relative_to(root)),'reason':'No stylesheet fixture mapping'})
 css+=(root/'assets/css/appearance.css').read_text().replace('__RUSTDL_RAINY_CITY_BACKGROUND__','/dev/rainy-city.webp').replace('__RUSTDL_RAINY_CITY_LIGHT_BACKGROUND__','/dev/rainy-city-light.webp')
 text=template.read_text();unmapped=[]
 def replace(m):
  key=m.group(1)
  if key in values:return values[key]
  if key.endswith('_CSS'):return ''
  if key in {'cards','rows'}:return '<p class="synthetic-empty">Synthetic empty state</p>'
  if any(part in key for part in ['count','number','page','total','MAX_']):return '3'
  if key.endswith('_size'):return '12 MB'
  unmapped.append(key);return 'Synthetic'
 text=re.sub(r'(?<!\{)\{([A-Za-z_][A-Za-z0-9_]*)\}(?!\})',replace,text)
 text=re.sub(r'<script\b[^>]*>.*?</script>','',text,flags=re.S)
 text=text.replace('{{','{').replace('}}','}')
 text=re.sub(r'<link\b[^>]*rel="stylesheet"[^>]*>','',text)
 if name in fragments:text='<html><head><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><main>'+text+'</main></body></html>'
 text=text.replace('</head>','<style>'+css+'</style><script>window.devErrors=[];window.devInterrupted=false;document.addEventListener("visibilitychange",()=>{if(document.hidden)window.devInterrupted=true});addEventListener("error",e=>devErrors.push(e.message));</script></head>')
 # Keep navigation and all form submissions inert in this synthetic smoke suite.
 text=text.replace('</body>','<script>document.addEventListener("submit",e=>e.preventDefault());</script></body>')
 filename='screen-'+name+'.html';(out/filename).write_text(text)
 for profile in ['w280','w300','w320','w344','w360','w375','w393','w412','w414','w428','w480','w540','w600','w640','w720','w768','w800','w900','w1024','w1152','w1280','w1366','w1440','w1536','w1600','w1920','w2560','w3440','w3840','w393-text150','w393-city-light','w393-city-dark','w393-landscape','w600-landscape','w768-landscape','w1024-landscape']:
  cases.append({'name':name+'/'+profile,'asset':filename,'profile':profile,'template':str(template.relative_to(root)),'coverage':'synthetic visual smoke only','sourceSha256':hashlib.sha256(template.read_bytes()).hexdigest()})
 if unmapped:missing.append({'target':str(template.relative_to(root)),'reason':'Specific fixture values missing: '+', '.join(sorted(set(unmapped)))})
 if '{cards}' in template.read_text() or '{rows}' in template.read_text():missing.append({'target':str(template.relative_to(root)),'reason':'Populated list states not covered by this empty-state fixture'})
# Populated batch cases execute production selection scripts with inert synthetic forms.
for name in ['playlist', 'quality', 'queue']:
 template=root/'assets/html'/f'{name}.html'
 items=[]
 for i in range(500):
  filename=f'youtube-batchx{i:05}.mp4'
  if name=='playlist':
   items.append(f'<label class="candidate"><input type="checkbox" name="pick" value="synthetic:{i}"><span class="check">✓</span><span class="candidate-copy"><strong><i>#{i+1}</i> Synthetic playlist video {i+1} with a longer title</strong><span>Artist {i%2} · Development fixture</span><code>{filename}</code></span></label>')
  elif name=='quality':
   options=''.join(f'<option value="synthetic:{i}:{q}" data-kind="{kind}" data-height="{height}">{label}</option>' for q,kind,height,label in [(0,'video',1080,'Best · 1080p'),(1,'video',720,'Balanced · 720p'),(2,'audio',0,'Audio only · M4A')])
   items.append(f'<article class="quality-card"><div><strong>Synthetic artist {i+1}</strong><code>{filename}</code></div><label>Format &amp; quality<select name="pick">{options}</select></label></article>')
  else:
   items.append(f'<article data-filename="{filename}" data-phase="queued"><div class="row"><div class="info"><span class="phase">Queued</span><code>{filename}</code><span class="size">0 B / 12 MB</span><span class="quality">Best · 1080p</span></div><nav><a href="#">Play</a><a href="#">Pause</a><a class="danger" href="#">Cancel</a></nav></div><div class="progress"></div></article>')
 replacements=dict(values, cards=''.join(items), rows=''.join(items), count='500', queue_script='', DISCOVERY_CSS='')
 text=re.sub(r'(?<!\{)\{([A-Za-z_][A-Za-z0-9_]*)\}(?!\})',lambda m:replacements.get(m.group(1),''),template.read_text())
 css=''.join((root/'assets/css'/f'{style}.css').read_text() for style in styles.get(name,[name]))
 text=text.replace('</head>','<style>'+css+'</style><script>window.devErrors=[];window.devInterrupted=false;addEventListener("error",e=>devErrors.push(e.message));document.addEventListener("visibilitychange",()=>{if(document.hidden)devInterrupted=true});</script></head>')
 behavior=(root/'assets/js/list-pagination.js').read_text()+'\n'+(root/'assets/js'/('playlist.js' if name=='playlist' else 'bulk-quality.js')).read_text() if name!='queue' else ''
 text=text.replace('</body>','<script>'+behavior+'</script><script>document.addEventListener("submit",e=>e.preventDefault());</script></body>')
 filename=f'screen-{name}-500.html';(out/filename).write_text(text)
 for profile in ['w280','w320','w360','w393','w412','w480','w540','w600','w720','w768','w900','w1024','w393-text150','w393-landscape','w600-landscape','w768-landscape','w1024-landscape']:
  cases.append({'name':f'{name}-500/{profile}','asset':filename,'profile':profile,'script':'visual-batch500.js','coverage':'500 synthetic entries; production styles and playlist/quality interactions; no network or media decoding'})
for script in sorted((root/'assets/js').glob('*.js')):
 if script.name!='playback.js':missing.append({'target':str(script.relative_to(root)),'reason':'Behavior/interactions not exercised by visual smoke fixtures'})
manifest=(root/'android/AndroidManifest.xml').read_text()
for name in re.findall(r'<activity\b[^>]*android:name="([^"]+)"',manifest,re.S):
 missing.append({'target':name,'reason':'Native activity lifecycle and integrations not exercised by dev WebView host'})
missing.append({'target':'assets/js/playback.js','reason':'Scroll batching covered; playback, sharing, deletion, and other actions not covered by visual scroll run'})
source_files=[p for pattern in ['assets/html/*.html','assets/css/*.css','assets/js/*.js','android/*.java','android/AndroidManifest.xml','src/dev/android/*.java','src/dev/visual-*.js','src/dev/*.py','android/build-termux.sh','assets/images/**/*.webp'] for p in root.glob(pattern)]
source_hashes={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in source_files}
data={'sourceHashes':source_hashes,'cases':cases,'missing':missing,'scope':'All discovered HTML templates; fixed 280/300/320/344/360/375/393/412/414/428/480/540/600/640/720/768/800/900/1024 CSS-pixel widths plus 150% text and city theme profiles; synthetic fixture smoke plus production gallery scroll. Not full behavioral coverage.'}
(out/'suite.json').write_text(json.dumps(data,indent=2))
print(f'DISCOVERED {len(cases)} visual cases from {len(cases)//2} HTML templates')
for row in missing:print('MISSING '+row['target']+': '+row['reason'])
