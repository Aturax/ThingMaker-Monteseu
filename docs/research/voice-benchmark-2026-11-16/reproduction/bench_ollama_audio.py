import json,pathlib,subprocess,os,time,requests
root=pathlib.Path(__file__).resolve().parent;host='http://127.0.0.1:11435';model='gemma4:12b'
env=os.environ.copy();env.update(OLLAMA_HOST='127.0.0.1:11435',OLLAMA_NO_CLOUD='1',OLLAMA_MODELS=r'C:\Users\franc\.ollama\models')
log=(root/'ollama-audio-runtime.log').open('w',encoding='utf8')
p=subprocess.Popen([r'C:\Users\franc\AppData\Local\Programs\Ollama\ollama.exe','serve'],env=env,stdout=log,stderr=log,creationflags=subprocess.CREATE_NO_WINDOW)
try:
 for _ in range(60):
  try:requests.get(host+'/api/version',timeout=1).raise_for_status();break
  except requests.RequestException:time.sleep(.5)
 for repeat in range(3):
  for clip in json.loads((root/'manifest.json').read_text(encoding='utf8')):
   t=time.perf_counter()
   with (root/clip['file']).open('rb') as f:
    r=requests.post(host+'/v1/audio/transcriptions',files={'file':(clip['file'],f,'audio/wav')},data={'model':model,'language':clip['language'],'response_format':'json','temperature':'0'},timeout=240)
   elapsed=time.perf_counter()-t
   record={'clip':clip['id'],'repeat':repeat,'model':model,'seconds':elapsed,'sha256':clip['sha256'],'status':r.status_code,'response':r.json()}
   with (root/'ollama-audio.jsonl').open('a',encoding='utf8') as f:f.write(json.dumps(record,ensure_ascii=False)+'\n')
   print(clip['id'],repeat,round(elapsed,3),r.status_code,r.text[:1200],flush=True)
   r.raise_for_status()
 (root/'ollama-audio-environment.json').write_text(json.dumps(requests.get(host+'/api/ps',timeout=10).json(),indent=2),encoding='utf8')
finally:
 try:requests.post(host+'/api/generate',json={'model':model,'keep_alive':0},timeout=10)
 except Exception:pass
 p.terminate()
 try:p.wait(timeout=5)
 except subprocess.TimeoutExpired:p.kill()
 log.close()
