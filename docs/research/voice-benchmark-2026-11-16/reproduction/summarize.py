import json,pathlib,re,statistics,hashlib
root=pathlib.Path(__file__).resolve().parent
dest=root.parents[1]/'docs/research/voice-benchmark-2026-11-16'
dest.mkdir(parents=True,exist_ok=True)
manifest=json.loads((root/'manifest.json').read_text(encoding='utf8'))
def words(s):
 s=s.lower().replace('quince','15').replace('cincuenta','50')
 return re.findall(r'\w+',s)
def distance(a,b):
 prev=list(range(len(b)+1))
 for i,x in enumerate(a,1):
  row=[i]
  for j,y in enumerate(b,1):row.append(min(row[-1]+1,prev[j]+1,prev[j-1]+(x!=y)))
  prev=row
 return prev[-1]
report={'scope':'synthetic file transcription; no microphone or human quality benchmark','manifest':manifest,'engines':{}}
files={'parakeet':'parakeet-results.jsonl','whisper_cpp':'whisper-cpp-results.jsonl','whisper_gpu':'whisper-server.jsonl','ollama_cleanup':'ollama-cleanup.jsonl','ollama_audio':'ollama-audio.jsonl'}
for engine,filename in files.items():
 rows=[json.loads(l) for l in (root/filename).read_text(encoding='utf8').splitlines()]
 report['engines'][engine]={'raw_runs':rows,'summary':[]}
 for clip in manifest:
  cr=[r for r in rows if r['clip']==clip['id']];warm=[r['seconds'] for r in cr if r['repeat']>0]
  text=cr[-1].get('text') or cr[-1].get('response',{}).get('raw') or cr[-1].get('response',{}).get('text')
  a,b=words(clip['reference']),words(text)
  summary={'clip':clip['id'],'warm_median_seconds':statistics.median(warm),'warm_min_seconds':min(warm),'warm_max_seconds':max(warm),'first_call_seconds':cr[0]['seconds'],'text':text,'word_errors':distance(a,b),'reference_words':len(a),'normalized_wer':distance(a,b)/len(a)}
  report['engines'][engine]['summary'].append(summary)
for f in ['parakeet-environment.json','whisper-cpp-environment.json','ollama-environment.json','ollama-audio-environment.json','ollama-audio-capabilities.json','codex-environment.json','codex-blocker.json','agy-capabilities.json','whisper-downloads.json']:
 if (root/f).exists():report[f]=json.loads((root/f).read_text(encoding='utf8'))
report['hardware']={'os':'Windows 11 build 26300','cpu':'AMD Ryzen 9 9950X3D2 16-Core Processor','logical_cpus':32,'physical_memory_bytes':66188959744,'gpu':'NVIDIA GeForce RTX 5090 (Ollama runtime log); NVML query failed'}
(dest/'results.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf8')
print(dest/'results.json')
for e,d in report['engines'].items():
 print(e,[(s['clip'],round(s['warm_median_seconds'],3),s['word_errors'],s['reference_words']) for s in d['summary']])
print('manifest integrity',all(hashlib.sha256((root/c['file']).read_bytes()).hexdigest()==c['sha256'] for c in manifest))
