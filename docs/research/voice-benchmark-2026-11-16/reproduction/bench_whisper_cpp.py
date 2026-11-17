import pathlib,subprocess,json,time,requests
root=pathlib.Path(__file__).resolve().parent
exe=root/'whisper-bin/Release/whisper-server.exe'
log=(root/'whisper-cpp-runtime.log').open('w',encoding='utf8')
t=time.perf_counter()
p=subprocess.Popen([str(exe),'-m',str(root/'ggml-small.bin'),'-t','4','-ng','--host','127.0.0.1','--port','11436','-bs','1','-bo','1'],cwd=root,stdout=log,stderr=log,creationflags=subprocess.CREATE_NO_WINDOW)
try:
 for _ in range(120):
  if p.poll() is not None:raise RuntimeError('whisper server exited')
  try:
   requests.get('http://127.0.0.1:11436/',timeout=.5);break
  except requests.RequestException:time.sleep(.1)
 else:raise RuntimeError('server did not start')
 load=time.perf_counter()-t;print('load',load,flush=True)
 (root/'whisper-cpp-environment.json').write_text(json.dumps({'load_seconds':load,'version':'v1.9.2','model':'small multilingual f16','threads':4,'gpu':False,'beam_size':1,'best_of':1},indent=2),encoding='utf8')
 for repeat in range(3):
  for clip in json.loads((root/'manifest.json').read_text(encoding='utf8')):
   t=time.perf_counter()
   with (root/clip['file']).open('rb') as f:
    r=requests.post('http://127.0.0.1:11436/inference',files={'file':(clip['file'],f,'audio/wav')},data={'language':clip['language'],'response_format':'json','temperature':'0','temperature_inc':'0'},timeout=120)
   elapsed=time.perf_counter()-t;r.raise_for_status();data=r.json()
   row={'clip':clip['id'],'repeat':repeat,'seconds':elapsed,'sha256':clip['sha256'],'response':data}
   with (root/'whisper-cpp-results.jsonl').open('a',encoding='utf8') as f:f.write(json.dumps(row,ensure_ascii=False)+'\n')
   print(clip['id'],repeat,round(elapsed,3),data,flush=True)
finally:
 p.terminate()
 try:p.wait(timeout=5)
 except subprocess.TimeoutExpired:p.kill()
 log.close()
