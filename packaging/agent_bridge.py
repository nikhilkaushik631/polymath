#!/usr/bin/env python3
"""Local stdio MCP bridge for the single Polymath application; standard library only."""
from pathlib import Path
import argparse,json,os,socket,subprocess,sys,time
ROOT=Path(__file__).resolve().parent.parent

def request(endpoint,method,params=None):
    data=json.loads(endpoint.read_text())
    if data.get('host')!='127.0.0.1':raise ValueError('Polymath control must use 127.0.0.1')
    with socket.create_connection(('127.0.0.1',int(data['port'])),timeout=5) as connection:
        connection.settimeout(50)
        connection.sendall((json.dumps({'id':1,'token':data['token'],'method':method,'params':params or {}})+'\n').encode())
        raw=connection.makefile('rb').readline(32*1024*1024)
        result=json.loads(raw)
    if not result.get('ok'):raise RuntimeError(result.get('error','Polymath did not complete this request'))
    return result.get('result')

def connect(endpoint,launch):
    if endpoint.exists():
        # A stale endpoint is not overwritten; choose a new path after checking the old process.
        request(endpoint,'app.inspect');return
    if not launch:raise RuntimeError(f'Start Polymath with --control {endpoint} first, or pass --launch')
    bundle=ROOT/'dist/Polymath.app/Contents/MacOS/Polymath'
    binary=bundle if bundle.is_file() else ROOT.parent/'reader/target/release/polymath'
    if not binary.is_file():raise RuntimeError('Build Polymath first: cargo build --locked --release')
    endpoint.parent.mkdir(parents=True,exist_ok=True)
    log=endpoint.with_suffix('.log')
    # An explicitly configured bridge may launch an agent-enabled Polymath window.
    with log.open('ab') as output:
        process=subprocess.Popen([str(binary),'--control',str(endpoint)],stdout=output,stderr=output,start_new_session=True)
    deadline=time.monotonic()+45
    while time.monotonic()<deadline:
        if process.poll() is not None:raise RuntimeError(f'Polymath exited during startup; see {log}')
        if endpoint.exists():request(endpoint,'app.inspect');return
        time.sleep(.2)
    raise RuntimeError('Polymath did not start in time')

WORKSPACES={'type':'string','enum':['home','reader','tools','writer','sheets','slides','images']}
def tool(name,description,properties,required=()):
    return {'name':name,'description':description,'inputSchema':{'type':'object','properties':properties,'required':list(required),'additionalProperties':False}}
TOOLS=[
    tool('polymath_inspect','Inspect the shared workspace and all unsaved-change indicators.',{}),
    tool('polymath_open','Open a local file in its viewer or editor. Images, Markdown and plain text open read-only in Reader; set edit=true to open Images or Writer. Does not discard unsaved Writer work.',{'path':{'type':'string'},'edit':{'type':'boolean'}},['path']),
    tool('polymath_switch','Switch the visible workspace while retaining the other documents.',{'workspace':WORKSPACES},['workspace']),
    tool('polymath_commands','List actual editor command schemas and enablement before authoring.',{'workspace':WORKSPACES},['workspace']),
    tool('polymath_execute','Run a documented editor command. Save explicitly and inspect the result. Reader uses UI command IDs; use polymath_call for its view options.',{'workspace':WORKSPACES,'command':{'type':'string'},'params':{'type':'object'}},['workspace','command']),
    tool('polymath_document','Inspect a document in one editor without converting it.',{'workspace':WORKSPACES},['workspace']),
    tool('polymath_screenshot','Capture only the Polymath window to an absolute PNG path; bring Polymath forward first.',{'path':{'type':'string'}},['path']),
    tool('polymath_call','Call the documented Polymath or active editor control protocol. Folder choice must be answered by the owner. Never use this to discard unrelated work.',{'method':{'type':'string'},'params':{'type':'object'}},['method']),
]
ROUTES={'polymath_inspect':'app.inspect','polymath_open':'app.open','polymath_switch':'app.workspace','polymath_commands':'engine.commands','polymath_execute':'engine.execute','polymath_document':'document.inspect','polymath_screenshot':'ui.screenshot'}

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--endpoint',type=Path,default=Path.home()/'Library/Application Support/Polymath/agent-control.json');parser.add_argument('--launch',action='store_true');parser.add_argument('--call');parser.add_argument('--params',default='{}');options=parser.parse_args()
    if options.call:
        connect(options.endpoint,options.launch)
        print(json.dumps(request(options.endpoint,options.call,json.loads(options.params)),indent=2));return
    for line in sys.stdin:
        message={}
        try:
            message=json.loads(line);method=message.get('method');ident=message.get('id')
            if ident is None:continue
            if method=='initialize':result={'protocolVersion':message.get('params',{}).get('protocolVersion','2024-11-05'),'capabilities':{'tools':{}},'serverInfo':{'name':'polymath-local','version':'0.2.0'}}
            elif method=='ping':result={}
            elif method=='tools/list':result={'tools':TOOLS}
            elif method=='tools/call':
                p=message.get('params',{});name=p.get('name');args=p.get('arguments',{})
                if isinstance(name,str) and name.startswith('iris_'):name='polymath_'+name[5:]
                if name not in ROUTES and name!='polymath_call':raise ValueError('Unknown Polymath tool')
                connect(options.endpoint,options.launch)
                value=request(options.endpoint,args['method'] if name=='polymath_call' else ROUTES[name],args.get('params',{}) if name=='polymath_call' else args)
                result={'content':[{'type':'text','text':json.dumps(value,ensure_ascii=False)}],'isError':False}
            else:
                print(json.dumps({'jsonrpc':'2.0','id':ident,'error':{'code':-32601,'message':'Method not found'}}),flush=True);continue
            print(json.dumps({'jsonrpc':'2.0','id':ident,'result':result}),flush=True)
        except Exception as error:
            if message.get('id') is not None:
                print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'content':[{'type':'text','text':str(error)}],'isError':True}}),flush=True)
if __name__=='__main__':main()
