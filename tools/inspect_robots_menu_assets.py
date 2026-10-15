"""Create contact sheets for extracted Robots EDB texture IDs, no external deps except Pillow."""
import base64
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

root=Path(__file__).resolve().parents[1]
src=root/"target/robots_menu_assets/h04/textures"
dst=root/"target/robots_menu_assets/audit"
dst.mkdir(parents=True,exist_ok=True)
files=sorted(src.glob("*.png"),key=lambda f:f.name)
out=[]
for index in range(0,len(files),25):
    chunk=files[index:index+25]
    sheet=Image.new("RGB",(920,640),(45,45,45))
    draw=ImageDraw.Draw(sheet)
    for i,f in enumerate(chunk):
        x=(i%5)*184
        y=(i//5)*128
        im=Image.open(f).convert("RGBA")
        opaque=Image.new("RGBA",im.size,(90,90,90,255));opaque.alpha_composite(im)
        opaque.thumbnail((173,100))
        xx=x+(184-opaque.width)//2
        yy=y+(100-opaque.height)//2
        sheet.paste(opaque.convert("RGB"),(xx,yy))
        uid=f.name.split("0x")[-1].split("]")[0]
        draw.text((x+8,y+105),uid,fill=(255,255,255))
        out.append(f"{uid}\t{im.width}\t{im.height}")
    jpg=dst/f"contact_{index//25}.jpg"
    sheet.save(jpg,quality=54,optimize=True)
    (dst/f"contact_{index//25}.b64.txt").write_text(base64.b64encode(jpg.read_bytes()).decode("ascii"),encoding="ascii")
(dst/"texture_dimensions.tsv").write_text("\n".join(out),encoding="utf-8")
selected=["86000035","8600003E","8600003F","8600001E","8600001F"]
preview=Image.new("RGB",(1500,900),(39,41,47))
painter=ImageDraw.Draw(preview)
for i,uid in enumerate(selected):
    f=next((f for f in files if uid in f.name),None)
    if f is None:continue
    rgba=Image.open(f).convert("RGBA")
    plate=Image.new("RGBA",rgba.size,(90,92,98,255))
    plate.alpha_composite(rgba)
    ratio=min(680/max(1,rgba.width),360/max(1,rgba.height))
    shown=plate.resize((int(rgba.width*ratio),int(rgba.height*ratio)))
    x=(i%2)*750+12;y=(i//2)*300+12
    preview.paste(shown.convert("RGB"),(x,y))
    painter.text((x,y+258),f"{uid} {rgba.width}x{rgba.height}",fill=(250,250,250))
preview.save(dst/"selected.jpg",quality=70,optimize=True)
(dst/"selected.b64.txt").write_text(base64.b64encode((dst/"selected.jpg").read_bytes()).decode("ascii"),encoding="ascii")
print(f"Contact sheets {((len(files)+24)//25)} textures={len(files)}")
