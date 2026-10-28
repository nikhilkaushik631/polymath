#!/usr/bin/env python3
"""Native integration check: synthetic files only, one Polymath process and window."""
from pathlib import Path
import hashlib,importlib.util,json,os,shutil,socket,stat,subprocess,time
from images_smoke import fixture as image_fixture
ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('bridge',ROOT/'packaging/agent_bridge.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
out=ROOT/'target/native-qa';out.mkdir(parents=True,exist_ok=True)
endpoint=out/f'control-{os.getpid()}.json'
fixtures={'reader':('reader','target/interface-scale/bookmarks.pdf'),'writer':('writer','target/agent-integration/agent-demo.docx'),'sheets':('sheets','target/agent-integration/agent-demo.xlsx'),'slides':('slides','target/agent-integration/agent-demo.pptx')}
for workspace,(repo,relative) in fixtures.items():
 source=ROOT.parent/repo/relative;destination=out/(workspace+source.suffix);shutil.copyfile(source,destination);fixtures[workspace]=destination
log=(out/'app.log').open('w');env=os.environ.copy();env.update(POLYMATH_NO_PREFS='1',PDFCRAFT_SYSTEM_FONTS='0')
binary=Path(os.environ['POLYMATH_QA_BINARY']) if os.environ.get('POLYMATH_QA_BINARY') else ROOT/'dist/Polymath.app/Contents/MacOS/Polymath'
if not binary.is_file():binary=ROOT.parent/'reader/target/release/polymath'
process=subprocess.Popen([str(binary),'--control',str(endpoint)],stdout=log,stderr=log,env=env)
def call(method,params=None):return bridge.request(endpoint,method,params)
def execute(workspace,command,params=None):return call('engine.execute',{'workspace':workspace,'command':command,'params':params or {}})
def inspect(workspace):return call('document.inspect',{'workspace':workspace})
def shot(name):
 if os.environ.get('POLYMATH_SKIP_SCREENSHOTS')=='1':return None
 for attempt in range(2):
  subprocess.run(['swift','-e',f'import AppKit; NSRunningApplication(processIdentifier: {process.pid})?.activate(options: [])'],capture_output=True,timeout=15)
  time.sleep(.7)
  try:return call('ui.screenshot',{'path':str(out/(name+'.png'))})
  except RuntimeError as error:
   if attempt or 'Screenshot timed out' not in str(error):raise
try:
 deadline=time.monotonic()+60
 while not endpoint.exists():
  if process.poll() is not None or time.monotonic()>deadline:raise RuntimeError('Polymath did not start; check app.log')
  time.sleep(.25)
 print('Started one Polymath process:',process.pid,flush=True)
 subprocess.run(['swift','-e',f'import AppKit; NSRunningApplication(processIdentifier: {process.pid})?.activate(options: [])'],capture_output=True,timeout=15)
 assert stat.S_IMODE(endpoint.stat().st_mode)==0o600
 data=json.loads(endpoint.read_text())
 with socket.create_connection(('127.0.0.1',data['port']),timeout=5) as bad:
  bad.sendall((json.dumps({'id':1,'token':'incorrect','method':'app.inspect'})+'\n').encode());assert json.loads(bad.makefile('r').readline())['ok'] is False
 assert call('app.inspect')['workspace']=='home'
 assert call('app.folders.list')['folders']==[]
 shot('home-light')
 for page in ('appearance','voice','files'):
  assert call('app.settings',{'page':page})=={'visible':True,'page':page}
  shot('settings-'+page+'-light')
 call('app.theme',{'dark':True});call('app.settings',{'page':'appearance'});shot('settings-appearance-dark')
 assert call('app.settings',{'visible':False})['visible'] is False
 shot('home-dark');call('app.theme',{'dark':False})
 assert call('app.workspace',{'workspace':'reader'})['workspace']=='reader'
 shot('reader-empty-light')
 assert call('app.workspace',{'workspace':'images'})['workspace']=='images'
 shot('images-library-light')
 assert call('app.workspace',{'workspace':'video'})['workspace']=='video'
 assert call('app.video.inspect')['clipCount']==0
 shot('video-studio-empty-light')
 call('app.workspace',{'workspace':'home'})
 image=out/'visual-audit-image.png';image_fixture(image)
 assert call('app.open',{'path':str(image)})['workspace']=='reader'
 deadline=time.monotonic()+20
 while call('document.inspect',{'workspace':'reader'}).get('state')=='loading':
  if time.monotonic()>deadline:raise RuntimeError('Image preview did not load')
  time.sleep(.2)
 assert call('document.inspect',{'workspace':'reader'})['state']=='ready'
 shot('image-viewer-light')
 assert call('app.image.fullscreen',{'enabled':True})['fullscreen'] is True
 time.sleep(1.5);shot('image-fullscreen')
 assert call('app.image.fullscreen',{'enabled':False})['fullscreen'] is False
 call('app.theme',{'dark':True});shot('image-viewer-dark');call('app.theme',{'dark':False})
 call('app.workspace',{'workspace':'home'})
 if shutil.which('ffmpeg'):
  video=out/'visual-audit-video.mp4';audio=out/'visual-audit-audio.wav'
  subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-f','lavfi','-i','testsrc2=size=640x360:rate=24','-f','lavfi','-i','sine=frequency=440:sample_rate=44100','-t','4','-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac','-shortest',str(video)],check=True,timeout=45)
  subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-f','lavfi','-i','sine=frequency=440:sample_rate=44100','-t','4','-c:a','pcm_s16le',str(audio)],check=True,timeout=20)
  for path,name in ((video,'video'),(audio,'audio')):
   assert call('app.open',{'path':str(path)})['workspace']=='reader'
   deadline=time.monotonic()+20
   while call('app.media.inspect')['state']=='loading':
    if time.monotonic()>deadline:raise RuntimeError(f'{name} did not load')
    time.sleep(.2)
   assert abs(call('app.media.volume',{'value':0.35})['queue']['volume']-0.35)<0.01
   assert abs(call('app.media.volume',{'value':1.0})['queue']['volume']-1.0)<0.01
   shot(name+'-viewer-light')
   call('app.theme',{'dark':True});shot(name+'-viewer-dark');call('app.theme',{'dark':False})
   assert call('app.media.fullscreen',{'enabled':True})['queue']['fullscreen'] is True
   time.sleep(1.5);shot(name+'-fullscreen')
   assert call('app.media.fullscreen',{'enabled':False})['queue']['fullscreen'] is False
  call('app.workspace',{'workspace':'home'})
 svg=out/'visual-audit-diagram.svg'
 svg.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="420" height="260" viewBox="0 0 420 260"><rect width="420" height="260" fill="#eef4f7"/><circle cx="118" cy="130" r="72" fill="#347d9c"/><path d="M118 130L252 78L324 182Z" fill="#e89a76"/></svg>',encoding='utf-8')
 assert call('app.open',{'path':str(svg)})['workspace']=='reader'
 assert '<svg' in call('document.inspect',{'workspace':'reader'})['text']
 shot('svg-code-preview-light')
 call('app.theme',{'dark':True});shot('svg-code-preview-dark');call('app.theme',{'dark':False})
 call('app.workspace',{'workspace':'home'})
 jsx=out/'visual-audit-component.jsx'
 jsx.write_text('import { motion } from "framer-motion";\nexport default function Slide() {\n'+'\n'.join(f'  const row{i} = <div data-row="{i}">Row {i}</div>;' for i in range(1,72))+'\n}\n',encoding='utf-8')
 assert call('app.open',{'path':str(jsx)})['workspace']=='reader'
 assert inspect('reader')['lines']==75
 long_tab=out/'march9_2026_addip_source_with_a_very_long_name.sh'
 long_tab.write_text('#!/bin/sh\nprintf "Polymath"\n',encoding='utf-8')
 assert call('app.open',{'path':str(long_tab)})['workspace']=='reader'
 shot('reader-tabs-many-light')
 assert call('app.reader.close',{'path':str(long_tab)})['closeRequested'] is True
 tabs=call('app.reader.tabs')['tabs']
 assert {str(svg),str(jsx)}.issubset({tab['path'] for tab in tabs}),tabs
 assert call('app.reader.select',{'path':str(svg)})['active']
 assert '<svg' in inspect('reader')['text']
 assert call('app.reader.select',{'path':str(jsx)})['active']
 assert inspect('reader')['path']==str(jsx)
 assert call('app.reader.close',{'path':str(svg)})['closeRequested'] is True
 assert str(svg) not in {tab['path'] for tab in call('app.reader.tabs')['tabs']}
 shot('source-long-jsx-light')
 call('app.theme',{'dark':True});shot('source-long-jsx-dark');call('app.theme',{'dark':False})
 assert call('app.reader.close')['closeRequested'] is True
 markdown=out/'notes.md';markdown.write_text('# Polymath Markdown check\n\nA **bold** idea and a [local link](#details).\n\n- First item\n- Second item\n',encoding='utf-8')
 csv=out/'table.csv';csv.write_text('Place,Amount\n"North, office",42\nSouth,17\n',encoding='utf-8')
 assert call('app.open',{'path':str(markdown)})['workspace']=='reader'
 assert inspect('reader')['kind']=='source'
 assert call('app.open',{'path':str(markdown),'edit':True})['workspace']=='writer'
 assert 'Polymath Markdown check' in json.dumps(inspect('writer'))
 assert call('app.open',{'path':str(csv)})['workspace']=='reader'
 assert inspect('reader')['kind']=='source'
 shot('csv-table-light')
 wide=out/'wide-table.csv'
 wide.write_text('id,cameraId,locationId,timestamp,licensePlate,vehicleType,make,model,color,direction\n'+'\n'.join(f'{index:08d},160466e5c2a44f2,{index:04d},2026-10-28T01:44:49Z,KA18EB42,Two Wheeler,Example,Commuter,silver,north' for index in range(40))+'\n',encoding='utf-8')
 assert call('app.open',{'path':str(wide)})['workspace']=='reader'
 assert inspect('reader')['kind']=='source'
 shot('csv-wide-table-light')
 assert call('app.open',{'path':str(csv),'edit':True})['workspace']=='sheets'
 assert execute('sheets','cell.get',{'cell':'A2'})['value']=='North, office'
 assert execute('sheets','cell.get',{'cell':'B2'})['value']==42
 print('PASS: Markdown content and quoted/numeric CSV cells.',flush=True)

 for ext in ('xltx','xltm'):
  template=out/f'template.{ext}';shutil.copyfile(fixtures['sheets'],template)
  assert call('app.open',{'path':str(template)})['workspace']=='reader'
  assert inspect('reader')['kind']=='officePreview'
  assert call('app.open',{'path':str(template),'edit':True})['workspace']=='sheets'
  assert inspect('sheets')
 print('PASS: Excel templates open in Sheets and preview in Reader.',flush=True)

 for name,content in [('example.py','def hello():\n    return "नमस्ते"\n'),('component.tsx','export const Card = () => <article>Hello</article>;\n'),('index.html','<!doctype html><title>Polymath</title>\n'),('configuration.toml','[reader]\nlocal = true\n'),('unknown.custom','Plain text still opens in Reader.\n')]:
  source=out/name;source.write_text(content,encoding='utf-8')
  assert call('app.open',{'path':str(source)})['workspace']=='reader'
  detail=inspect('reader')
  assert detail['kind']=='source' and detail['text']==content and detail['path']==str(source),detail
  assert call('app.inspect')['source']['path']==str(source)
  if name=='example.py':shot('source-outline-light')
 binary=out/'not-text.custom';binary.write_bytes(b'\x00\xff\x00\xff')
 try:call('app.open',{'path':str(binary)});raise AssertionError('binary file was accepted as source text')
 except RuntimeError as error:assert 'text' in str(error).lower() or 'binary' in str(error).lower(),error
 shot('source-light');call('app.theme',{'dark':True});shot('source-dark');call('app.theme',{'dark':False})
 plain=out/'notes.txt';plain.write_text('Local plain-text editing.\n',encoding='utf-8')
 for path in [markdown,plain]:
  assert call('app.preview',{'path':str(path)})['workspace']=='reader'
  detail=inspect('reader');assert detail['kind']=='source' and detail['path']==str(path),detail
  assert call('app.open',{'path':str(path)})['workspace']=='reader'
  assert call('app.open',{'path':str(path),'edit':True})['workspace']=='writer'
  assert ('Polymath Markdown check' if path.suffix=='.md' else 'Local plain-text editing') in json.dumps(inspect('writer'))
 for workspace in ['writer','sheets','slides']:
  path=fixtures[workspace];original=hashlib.sha256(path.read_bytes()).hexdigest()
  assert call('app.preview',{'path':str(path)})['readOnly'] is True
  detail=inspect('reader');assert detail['kind']=='officePreview' and detail['path']==str(path),detail
  assert hashlib.sha256(path.read_bytes()).hexdigest()==original
  try:execute('reader','page.rotate');raise AssertionError('office preview accepted a PDF edit')
  except RuntimeError as error:assert 'read-only' in str(error),error
  shot('preview-'+workspace+'-light')
 print('PASS: DOCX, XLSX and PPTX render in Reader without changing their originals; Markdown and text open in Reader and edit in Writer.',flush=True)
 assert call('app.workspace',{'workspace':'tools'})['workspace']=='tools'
 shot('pdf-tools-light');call('app.workspace',{'workspace':'home'})
 print('PASS: source files open read-only in Reader; PDF Tools have a separate workspace.',flush=True)

 states={}
 for workspace,path in fixtures.items():
  result=call('app.open',{'path':str(path)});assert result['workspace']=='reader',result
  if workspace!='reader':
   result=call('app.open',{'path':str(path),'edit':True});assert result['workspace']==workspace,result
  time.sleep(1)
  states[workspace]=inspect(workspace)
  assert states[workspace],states
  shot(workspace+'-light')
  call('app.theme',{'dark':True});shot(workspace+'-dark');call('app.theme',{'dark':False})
  print('Opened and inspected:',workspace,flush=True)
 original_pdf_hash=hashlib.sha256(fixtures['reader'].read_bytes()).hexdigest()
 assert call('app.reader.select',{'path':str(fixtures['reader'])})['active']
 execute('reader','page.rotate')
 execute('writer','file.autosave',{'value':False})
 execute('writer','text.insert',{'text':'Polymath unified workspace verification. '})
 execute('sheets','cell.set',{'cell':'A9','input':'43'})
 execute('slides','slide.new')
 before={w:inspect(w) for w in ['writer','sheets','slides']}
 for _ in range(2):
  for workspace in ['home','reader','writer','sheets','slides']:
   call('app.workspace',{'workspace':workspace});time.sleep(.2)
 assert 'Polymath unified workspace verification' in json.dumps(inspect('writer'))
 assert execute('sheets','cell.get',{'cell':'A9'})['value']==43
 assert len(inspect('slides')['slides'])==len(before['slides']['slides'])
 call('app.workspace',{'workspace':'home'})
 result=call('app.quit');assert set(result['dirtyWorkspaces'])>={'reader','writer','sheets','slides'},result
 time.sleep(.5);assert process.poll() is None;assert call('app.inspect')['quitConfirmation'];shot('unsaved-changes');call('app.quit.cancel')
 execute('reader','file.save')
 deadline=time.monotonic()+15
 while 'reader' in call('app.inspect')['dirtyWorkspaces']:
  if time.monotonic()>deadline:raise RuntimeError('PDF did not save')
  time.sleep(.2)
 assert hashlib.sha256(fixtures['reader'].read_bytes()).hexdigest()!=original_pdf_hash
 saved_pdf=out/'saved-reader.pdf';shutil.copyfile(fixtures['reader'],saved_pdf);call('app.open',{'path':str(saved_pdf)});time.sleep(.5);assert inspect('reader')
 for workspace,ext in [('writer','docx'),('sheets','xlsx'),('slides','pptx')]:
  saved=out/f'saved-{workspace}.{ext}';execute(workspace,'file.saveAs',{'path':str(saved)});assert saved.is_file() and saved.stat().st_size>0
  call('app.open',{'path':str(saved),'edit':True});assert inspect(workspace)
 assert 'Polymath unified workspace verification' in json.dumps(inspect('writer'))
 assert execute('sheets','cell.get',{'cell':'A9'})['value']==43
 call('app.workspace',{'workspace':'home'});shot('home-recent')
 (out/'inspection.json').write_text(json.dumps({'initial':states,'edited':before,'final':call('app.inspect')},indent=2))
 assert call('app.inspect')['dirtyWorkspaces']==[]
 call('app.quit');process.wait(timeout=15)
 assert process.returncode==0,process.returncode
 print('PASS: routing, light/dark rendering, repeated switches, inactive-document close guard, save and reopen.',flush=True)
finally:
 if process.poll() is None:process.terminate();process.wait(timeout=15)
 endpoint.unlink(missing_ok=True);log.close()
