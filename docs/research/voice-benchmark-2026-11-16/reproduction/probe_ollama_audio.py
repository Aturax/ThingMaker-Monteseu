import pathlib,subprocess,os,requests,time,json
root=pathlib.Path(__file__).resolve().parent
env=os.environ.copy();env.update(OLLAMA_HOST='127.0.0.1:11435',OLLAMA_NO_CLOUD='1',OLLAMA_MODELS=r'C:\Users\franc\.ollama\models')
p=subprocess.Popen([r'C:\Users\franc\AppData\Local\Programs\Ollama\ollama.exe','serve'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
try:
 for _ in range(40):
  try:tags=requests.get('http://127.0.0.1:11435/api/tags',timeout=1).json()['models'];break
  except requests.RequestException:time.sleep(.25)
 out=[]
 for m in tags:
  d=requests.post('http://127.0.0.1:11435/api/show',json={'model':m['name']},timeout=10).json()
  out.append({'name':m['name'],'capabilities':d.get('capabilities',[])})
 (root/'ollama-audio-capabilities.json').write_text(json.dumps(out,indent=2),encoding='utf8');print(json.dumps(out))
finally:
 p.terminate()
 try:p.wait(timeout=5)
 except subprocess.TimeoutExpired:p.kill()
