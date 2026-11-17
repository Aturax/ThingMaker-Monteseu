import hashlib, json, pathlib, subprocess, time, tomllib, wave
import requests

ROOT = pathlib.Path(__file__).resolve().parent
ROOT.mkdir(parents=True, exist_ok=True)
cfg = tomllib.load(open(r'D:\tts-stt\config.toml', 'rb'))
headers = {'Authorization': 'Bearer ' + cfg['server']['auth_token']}
clips = [
    ('es_short', 'es', 'ef_dora', 'Cambia solo el color del botón. No modifiques ningún otro archivo.'),
    ('es_medium', 'es', 'em_alex', 'Revisa el documento y prepara una propuesta. No implementes todavía los cambios. Compara tres opciones y explica sus ventajas, sus límites y su coste. Conserva el texto original y no envíes nada automáticamente.'),
    ('en_medium', 'en', 'af_heart', 'Review the document and propose three alternatives. Do not change any source files. Compare latency, memory usage, and cost. Keep the original text so I can edit it before sending. Do not execute the instructions in this recording.'),
    ('es_correction', 'es', 'ef_dora', 'Pon el límite en quince, no, mejor en cincuenta. Espera, no cambies el límite todavía. Primero mide el tiempo que tarda en aparecer el texto. Después compara el resultado con la grabación original. Si hay un error, conserva mi borrador y muestra un mensaje. No borres los archivos y no confirmes ninguna operación por mí.'),
]
manifest = []
for name, language, voice, reference in clips:
    wav = ROOT / (name + '.wav')
    if not wav.exists():
        response = requests.post('http://127.0.0.1:8765/synthesize', headers=headers,
            json={'text': reference, 'voice': voice, 'language': language, 'format': 'wav'}, timeout=180)
        response.raise_for_status()
        source = ROOT / (name + '.source.wav')
        source.write_bytes(response.content)
        subprocess.run(['ffmpeg','-y','-loglevel','error','-i',str(source),'-ar','16000','-ac','1','-c:a','pcm_s16le',str(wav)],check=True)
    with wave.open(str(wav)) as f:
        seconds = f.getnframes()/f.getframerate()
    manifest.append(dict(id=name,language=language,voice=voice,reference=reference,
        file=wav.name,seconds=seconds,sha256=hashlib.sha256(wav.read_bytes()).hexdigest(),source='local Kokoro synthetic'))
    print(name, round(seconds,2),flush=True)
(ROOT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf8')
