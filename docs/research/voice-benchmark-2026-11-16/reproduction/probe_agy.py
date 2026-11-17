import json,subprocess,threading,queue,pathlib,shutil,time
root=pathlib.Path(__file__).resolve().parent
p=subprocess.Popen([shutil.which('agy'),'acp','--disable-telemetry'],cwd=root,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,encoding='utf8')
q=queue.Queue()
def read():
 for line in p.stdout:
  try:q.put(json.loads(line))
  except ValueError:pass
threading.Thread(target=read,daemon=True).start()
try:
 p.stdin.write(json.dumps({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':1,'clientCapabilities':{},'clientInfo':{'name':'thingmaker-voice-benchmark','version':'0.1'}}})+'\n');p.stdin.flush()
 end=time.perf_counter()+30
 while time.perf_counter()<end:
  d=q.get(timeout=max(.1,end-time.perf_counter()))
  if d.get('id')==1:
   r=d.get('result',{});summary={k:r.get(k) for k in ['protocolVersion','agentInfo','agentCapabilities']};summary['error']=d.get('error')
   (root/'agy-capabilities.json').write_text(json.dumps(summary,indent=2),encoding='utf8');print(json.dumps(summary));break
except Exception as e:print(type(e).__name__,str(e)[:300])
finally:
 p.terminate()
 try:p.wait(timeout=5)
 except subprocess.TimeoutExpired:p.kill()
