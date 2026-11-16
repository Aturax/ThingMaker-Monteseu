import json, pathlib, time, tomllib, requests
root=pathlib.Path(__file__).resolve().parent
clips=json.loads((root/'manifest.json').read_text(encoding='utf8'))
cfg=tomllib.load(open(r'D:\tts-stt\config.toml','rb'))
headers={'Authorization':'Bearer '+cfg['server']['auth_token']}
out=root/'whisper-server.jsonl'
health=requests.get('http://127.0.0.1:8765/health',timeout=5).json()
(root/'whisper-health-before.json').write_text(json.dumps(health,indent=2),encoding='utf8')
for repeat in range(3):
 for clip in clips:
  t=time.perf_counter()
  with (root/clip['file']).open('rb') as f:
   r=requests.post('http://127.0.0.1:8765/transcribe',headers=headers,
    files={'audio':(clip['file'],f,'audio/wav')},data={'cleanup':'false','language':clip['language']},timeout=120)
  elapsed=time.perf_counter()-t
  r.raise_for_status()
  result={'engine':'faster-whisper-large-v3-cuda-server','clip':clip['id'],'repeat':repeat,
   'seconds':elapsed,'audio_seconds':clip['seconds'],'sha256':clip['sha256'],
   'cold_model':repeat==0 and clip==clips[0] and not health.get('whisper_loaded'),
   'response':r.json(),'server_timing':r.headers.get('Server-Timing')}
  with out.open('a',encoding='utf8') as f:f.write(json.dumps(result,ensure_ascii=False)+'\n')
  print(clip['id'],repeat,round(elapsed,3),r.json().get('raw'),flush=True)
