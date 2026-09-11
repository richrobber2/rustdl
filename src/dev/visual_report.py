"""Write a standalone, escaped HTML review report from synthetic visual results."""
import base64, html, json
from pathlib import Path

def write_report(report, output, screenshot_dir):
    esc=lambda value:html.escape(str(value))
    sections=[]
    for case in report.get('visualCases',[]):
        audit=case.get('audit',{})
        rows=[]
        for issue in audit.get('issues',[]):
            rows.append('<tr><td>'+esc(issue.get('severity',''))+'</td><td>'+esc(issue.get('type',''))+'</td><td><code>'+esc(issue.get('selector',''))+'</code><br>'+esc(issue.get('label',''))+'</td><td><pre>'+esc(json.dumps({k:v for k,v in issue.items() if k not in ['label','selector','type','severity']},indent=2))+'</pre></td></tr>')
        screenshot=''
        name=case.get('screenshot','').lstrip('/')
        if name.startswith('failure-') and '/' not in name and name.endswith('.png'):
            path=Path(screenshot_dir)/name
            if path.exists():screenshot='<details><summary>Failure screenshot</summary><img alt="Synthetic failing test window" src="data:image/png;base64,'+base64.b64encode(path.read_bytes()).decode()+'"></details>'
        sections.append('<section><h2>'+('PASS' if case.get('pass') else 'FAIL')+' · '+esc(case.get('name',''))+'</h2><p>'+esc(case.get('coverage',''))+'</p><p>'+esc(audit.get('counts',{}))+' · omitted details: '+esc(audit.get('omitted',0))+'</p>'+(''.join('<p class=\"review\"><strong>Review coverage gap:</strong> '+esc(g.get('selector',''))+' — '+esc(g.get('reason',''))+'</p>' for g in audit.get('missingCoverage',[])))+screenshot+'<table><thead><tr><th>Severity</th><th>Issue</th><th>Element</th><th>Geometry / reason</th></tr></thead><tbody>'+''.join(rows)+'</tbody></table></section>')
    gaps=''.join('<li>'+esc(gap.get('target',''))+': '+esc(gap.get('reason',''))+'</li>' for gap in report.get('missingCoverage',[]))
    text='<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>RustDL visual audit</title><style>body{font:16px system-ui;margin:24px;color:#172c40;background:#eef3f7}section{background:white;padding:20px;margin:20px 0;border-radius:12px}.review{padding:.6rem;border-left:4px solid #d58b00;background:#fff4d6}table{border-collapse:collapse;width:100%}td,th{padding:8px;border:1px solid #ccd6de;text-align:left;vertical-align:top;overflow-wrap:anywhere}pre{white-space:pre-wrap;font-size:12px}img{max-width:100%;max-height:900px}code{word-break:break-all}</style><h1>RustDL visual audit</h1><p>Real WebView geometry checks. CSS ellipses are warnings; annotated truncation is recorded as allowed. A pass does not establish visual polish, contrast, or full functional coverage.</p>'+''.join(sections)+'<h2>Coverage gaps</h2><ul>'+gaps+'</ul></html>'
    Path(output).write_text(text)
