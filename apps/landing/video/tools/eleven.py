import json, sys, subprocess, os, urllib.request
req = json.load(open(sys.argv[1], encoding='utf-8')); voice = sys.argv[2]
key = os.environ["ELEVENLABS_API_KEY"]
for l in req['lines']:
    body = json.dumps({"text": l['text'], "model_id": "eleven_multilingual_v2",
        "voice_settings": {"stability": 0.5, "similarity_boost": 0.75, "style": 0.15, "speed": 1.05}}).encode()
    r = urllib.request.Request(f"https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=mp3_44100_128", data=body,
        headers={"xi-api-key": key, "Content-Type": "application/json"})
    out = f"assets/voice/{l['id']}.mp3"
    open(out, 'wb').write(urllib.request.urlopen(r).read())
    d = subprocess.run(['ffprobe','-v','error','-show_entries','format=duration','-of','csv=p=0',out],capture_output=True,text=True).stdout.strip()
    print(l['id'], d)
