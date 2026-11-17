import pathlib,requests,zipfile,hashlib,json,time
root=pathlib.Path(__file__).resolve().parent
downloads=[('whisper.zip','https://github.com/ggml-org/whisper.cpp/releases/download/v1.9.2/whisper-bin-x64.zip'),('ggml-small.bin','https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin')]
records=[]
for name,url in downloads:
 path=root/name
 if not path.exists():
  with requests.get(url,stream=True,timeout=60) as r:
   r.raise_for_status()
   with path.with_suffix('.partial').open('wb') as f:
    for chunk in r.iter_content(1024*1024):f.write(chunk)
  path.with_suffix('.partial').replace(path)
 records.append({'file':name,'url':url,'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
 print(name,path.stat().st_size,flush=True)
with zipfile.ZipFile(root/'whisper.zip') as z:z.extractall(root/'whisper-bin')
(root/'whisper-downloads.json').write_text(json.dumps(records,indent=2),encoding='utf8')
