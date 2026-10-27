#!/usr/bin/env python3
"""Check Finder file delivery and the stdio MCP bridge against the owned QA instance."""
from pathlib import Path
import importlib.util,json,os,subprocess,time
ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('bridge',ROOT/'packaging/agent_bridge.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
out=ROOT/'target/native-qa';endpoint=out/f'bundle-control-{os.getpid()}.json';bundle=ROOT/'dist/Polymath.app'
subprocess.run(['/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister','-f',str(bundle)],check=True)
env=os.environ.copy();env.update(POLYMATH_NO_PREFS='1',PDFCRAFT_SYSTEM_FONTS='0')
log=(out/'bundle.log').open('w');process=subprocess.Popen([str(bundle/'Contents/MacOS/Polymath'),'--control',str(endpoint)],stdout=log,stderr=log,env=env)
def call(method,params=None):return bridge.request(endpoint,method,params)
try:
 deadline=time.monotonic()+45
 while not endpoint.exists():
  if process.poll() is not None or time.monotonic()>deadline:raise RuntimeError('Bundle failed to start')
  time.sleep(.2)
 for filename in ['writer.docx','sheets.xlsx','slides.pptx','reader.pdf','notes.md','table.csv','example.py','component.tsx','index.html']:
  path=out/filename
  subprocess.run(['open','-a',str(bundle),str(path)],check=True)
  deadline=time.monotonic()+15
  while call('app.inspect')['workspace']!='reader' or not any(tab['path']==str(path) for tab in call('app.reader.tabs')['tabs']):
   if time.monotonic()>deadline:raise AssertionError(f'Finder did not deliver {path}')
   time.sleep(.2)
  state=call('document.inspect',{'workspace':'reader'});assert state
  print('Finder Open With: Reader tab',filename,flush=True)
 messages=[{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2024-11-05'}},{'jsonrpc':'2.0','id':2,'method':'tools/list'},{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'polymath_document','arguments':{'workspace':'reader'}}},{'jsonrpc':'2.0','id':4,'method':'tools/call','params':{'name':'polymath_switch','arguments':{'workspace':'tools'}}}]
 result=subprocess.run(['python3',str(ROOT/'packaging/agent_bridge.py'),'--endpoint',str(endpoint)],input=''.join(json.dumps(m)+'\n' for m in messages),text=True,capture_output=True,check=True,timeout=60)
 replies=[json.loads(line) for line in result.stdout.splitlines()];assert len(replies)==4
 assert len(replies[1]['result']['tools'])==8
 switch=next(tool for tool in replies[1]['result']['tools'] if tool['name']=='polymath_switch')
 assert 'tools' in switch['inputSchema']['properties']['workspace']['enum']
 assert replies[2]['result']['isError'] is False,replies[2]
 assert 'reader' in replies[2]['result']['content'][0]['text']
 assert replies[3]['result']['isError'] is False,replies[3]
 assert call('app.inspect')['workspace']=='tools'
 tools_state=call('ui.state',{'workspace':'tools'})
 assert tools_state['mode']=='AllTools' and tools_state['left_panel']=='AllTools',tools_state
 summary=call('app.inspect');print('Before closing:',summary['dirtyWorkspaces'],flush=True)
 (out/'finder-state.json').write_text(json.dumps({'app':summary,'details':{w:call('ui.inspect',{'workspace':w}) for w in summary['dirtyWorkspaces']}},indent=2))
 assert summary['dirtyWorkspaces']==[],summary
 call('app.quit');process.wait(timeout=15);assert process.returncode==0
 print('PASS: one native bundle, nine Finder routes, MCP initialize/list/document and PDF Tools switch.',flush=True)
finally:
 if process.poll() is None:process.terminate();process.wait(timeout=15)
 endpoint.unlink(missing_ok=True);log.close()
