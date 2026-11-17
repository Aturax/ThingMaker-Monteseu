import pathlib,sys,json,time,wave,platform,os
root=pathlib.Path(__file__).resolve().parent
sys.path.insert(0,str(root/'deps'))
import sherpa_onnx,numpy as np
folder=root/'sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8'
t=time.perf_counter()
recognizer=sherpa_onnx.OfflineRecognizer.from_transducer(encoder=str(folder/'encoder.int8.onnx'),decoder=str(folder/'decoder.int8.onnx'),joiner=str(folder/'joiner.int8.onnx'),tokens=str(folder/'tokens.txt'),num_threads=4,provider='cpu',model_type='nemo_transducer')
load=time.perf_counter()-t
print('load',load,flush=True)
(root/'parakeet-environment.json').write_text(json.dumps({'load_seconds':load,'runtime':sherpa_onnx.__version__,'threads':4,'platform':platform.platform(),'logical_cpus':os.cpu_count()},indent=2),encoding='utf8')
clips=json.loads((root/'manifest.json').read_text(encoding='utf8'))
for repeat in range(3):
 for clip in clips:
  with wave.open(str(root/clip['file'])) as f: samples=np.frombuffer(f.readframes(f.getnframes()),dtype=np.int16).astype(np.float32)/32768
  t=time.perf_counter(); stream=recognizer.create_stream();stream.accept_waveform(16000,samples);recognizer.decode_stream(stream)
  result={'engine':'parakeet-v3-int8-sherpa-cpu','clip':clip['id'],'repeat':repeat,'seconds':time.perf_counter()-t,'audio_seconds':clip['seconds'],'sha256':clip['sha256'],'text':stream.result.text}
  with (root/'parakeet-results.jsonl').open('a',encoding='utf8') as f:f.write(json.dumps(result,ensure_ascii=False)+'\n')
  print(clip['id'],repeat,round(result['seconds'],3),result['text'],flush=True)
