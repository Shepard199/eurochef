from pathlib import Path
import json, collections, base64
from PIL import Image, ImageDraw

root=Path(__file__).resolve().parents[1]/'target/robots_menu_assets'
scripts=root/'h04/scripts'
for uid in ('0400027D','040002AE','04000035','040002EC','04000010','04000009'):
    fs=list(scripts.glob(f'*{uid}*'))
    if not fs: continue
    d=json.loads(fs[0].read_text())
    cmds=d.get('commands',[])
    print('\nSCRIPT',uid,'keys',list(d),'commands',len(cmds),
      'opcode',dict(collections.Counter(c.get('opcode') for c in cmds)),
      'time',(min((c.get('start',0) for c in cmds),default=0),
              max((c.get('start',0)+c.get('length',0) for c in cmds),default=0)))
    for c in cmds:
        data=c.get('data',{})
        if any(k in data for k in ('SubScript','Entity','Animation','Event')):
            sub=next(iter(data))
            print(c['start'],c['length'],c.get('controller_index'),sub,str(data[sub])[:120])
            if c['start']>190: break

src=root/'fonts/textures'
files=list(sorted(src.glob('*.png')))
canvas=Image.new('RGB',(1200,((len(files)+3)//4)*180),'#37383c')
draw=ImageDraw.Draw(canvas)
for j,f in enumerate(files):
    im=Image.open(f).convert('RGBA')
    bg=Image.new('RGBA',im.size,(90,90,90,255))
    bg.alpha_composite(im)
    bg.thumbnail((285,146))
    x=(j%4)*300;y=(j//4)*180
    canvas.paste(bg.convert('RGB'),(x,y))
    draw.text((x+8,y+147),f.stem[-20:],fill='white')
    print('FONT',f.name,im.size)
out=root/'audit/font_contact.jpg';out.parent.mkdir(parents=True,exist_ok=True)
canvas.save(out,quality=55)
out.with_suffix('.b64.txt').write_text(base64.b64encode(out.read_bytes()).decode())
