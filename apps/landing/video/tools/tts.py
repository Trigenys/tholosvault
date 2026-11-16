import asyncio, json, sys, subprocess
import edge_tts
req = json.load(open(sys.argv[1], encoding='utf-8'))
voice = sys.argv[2]; rate = sys.argv[3]
async def run():
    for l in req['lines']:
        out = f"assets/voice/{l['id']}.mp3"
        await edge_tts.Communicate(l['text'], voice, rate=rate).save(out)
        d = subprocess.run(['ffprobe','-v','error','-show_entries','format=duration','-of','csv=p=0',out],capture_output=True,text=True).stdout.strip()
        print(l['id'], d)
asyncio.run(run())
