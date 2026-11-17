import pathlib, requests, tarfile, time, hashlib
root=pathlib.Path(__file__).resolve().parent
url='https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2'
archive=root/'parakeet.tar.bz2'
if not archive.exists():
 t=time.perf_counter(); n=0
 with requests.get(url,stream=True,timeout=60) as r:
  r.raise_for_status()
  with archive.with_suffix('.partial').open('wb') as f:
   for chunk in r.iter_content(1024*1024):
    f.write(chunk); n+=len(chunk)
 archive.with_suffix('.partial').replace(archive)
 print('download',n,round(time.perf_counter()-t,2),flush=True)
print('sha256',hashlib.sha256(archive.read_bytes()).hexdigest(),flush=True)
with tarfile.open(archive) as t:t.extractall(root,filter='data')
print('extracted',flush=True)
