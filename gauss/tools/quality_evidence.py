#!/usr/bin/env python3
"""Assemble real GPU outputs; never retouch, sharpen, relight or synthesize them.
Requires Pillow and fontconfig. Run after the ignored GPU export tests.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
from PIL import Image, ImageChops, ImageDraw, ImageFont, ImageStat


def fonts():
    path = subprocess.check_output(
        ["fc-match", "-f", "%{file}", "Liberation Sans"], text=True
    ).strip()
    return {size: ImageFont.truetype(path, size) for size in (17, 20, 25)}


def comparisons(root, font):
    """RMS against same-renderer 4x-per-axis box integration, not ground truth."""
    assets = []
    for directory in sorted(root.iterdir()):
        if not directory.is_dir() or directory.name == "preceding-pass":
            continue
        if not (directory / "overview-sample4x.png").exists():
            continue
        results = {}
        for view in ("overview", "detail"):
            reference = Image.open(directory / f"{view}-sample4x.png").convert("RGB")
            assert reference.size == (1000, 760)
            mask = Image.eval(reference.convert("L"), lambda x: 255 if x > 10 else 0)
            results[view] = {}
            canvas = Image.new("RGB", (1500, 642), (9, 12, 18))
            draw = ImageDraw.Draw(canvas)
            for col, mode in enumerate(("reference", "balanced", "ultra")):
                im = Image.open(directory / f"{view}-{mode}.png").convert("RGB")
                assert im.size == reference.size
                stat = ImageStat.Stat(ImageChops.difference(im, reference), mask)
                assert sum(stat.count) > 0, "comparison requires nonempty foreground"
                rms = math.sqrt(sum(stat.sum2) / sum(stat.count))
                results[view][mode] = {
                    "mae": sum(stat.mean) / 3,
                    "rmse": rms,
                    "psnr": 20 * math.log10(255 / rms) if rms else None,
                }
                canvas.paste(im.resize((500, 380), Image.Resampling.LANCZOS), (col * 500, 32))
                canvas.paste(im.crop((350, 280, 850, 480)), (col * 500, 438))
                draw.text((col * 500 + 14, 7), f"{mode.upper()} / same camera", font=font[20], fill=(210,225,220))
                draw.text((col * 500 + 14, 416), "1:1 crop, no sharpening", font=font[17], fill=(148,180,169))
            canvas.save(directory / f"{view}-comparison.png")
        manifest = {}
        for path in sorted(directory.glob("*.png")):
            manifest[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
        payload = {
            "projection": "factored",
            "measurement": "RGB error against same renderer at 4x-per-axis sampling",
            "mask": "4x reference luminance >10",
            "source": (directory / "source.txt").read_text() if (directory / "source.txt").exists() else "See factor-sh3.log; 1,060,619 Gaussians / SH3",
            "views": results,
            "image_sha256": manifest,
        }
        (directory / "metrics.json").write_text(json.dumps(payload, indent=2) + "\n")
        for view, values in results.items():
            print(directory.name, view, "RMSE", {mode: round(v["rmse"],3) for mode,v in values.items()})
        assets.append(directory.name)
    return assets


def motion(root, font):
    out = root / "frames"
    out.mkdir(exist_ok=True)
    for frame in range(144):
        canvas = Image.new("RGB", (1488, 680), (9, 12, 18))
        draw = ImageDraw.Draw(canvas)
        draw.text((16,12), "GAUSS / GLASS - same-camera GPU motion comparison", font=font[25], fill=(216,232,224))
        for mode, x, label in (
            ("balanced",16,"BALANCED / native FP16"),
            ("ultra",752,"ULTRA / four samples per output pixel"),
        ):
            draw.text((x,53),label,font=font[20],fill=(155,221,190))
            im=Image.open(root/"raw"/f"{mode}-{frame:04}.png").convert("RGB")
            assert im.size == (720,540)
            canvas.paste(im,(x,84))
        draw.text((16,642),"Offline GPU renders | 270,491 Gaussians | 24 fps playback, NOT a real-time FPS claim",font=font[17],fill=(147,166,175))
        canvas.save(out/f"{frame:04}.png")
    for a,b in ((0,11),(108,143)):
        assert ImageChops.difference(Image.open(out/f"{a:04}.png"),Image.open(out/f"{b:04}.png")).getbbox() is None
    assert ImageChops.difference(Image.open(out/"0012.png"),Image.open(out/"0107.png")).getbbox() is not None
    for a,b in ((11,12),(107,108)):
        diff=ImageStat.Stat(ImageChops.difference(Image.open(out/f"{a:04}.png"),Image.open(out/f"{b:04}.png")))
        assert max(diff.mean)<1, (a,b,diff.mean)
    sheet=Image.new("RGB",(1488,1440),(9,12,18))
    draw=ImageDraw.Draw(sheet)
    for i,n in enumerate((0,11,12,60,107,108,143)):
        im=Image.open(out/f"{n:04}.png").resize((744,340),Image.Resampling.LANCZOS)
        x=(i%2)*744; y=(i//2)*360
        sheet.paste(im,(x,y));draw.text((x+12,y+340),f"global/source frame {n}",font=font[17],fill=(210,220,210))
    sheet.save(root/"contact-sheet.jpg",quality=93)
    print("MOTION_LAYOUT_OK 144 frames; exact quiet holds; adjacent hold boundaries mean error <1/255")


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--motion",action="store_true",help="also assemble previously exported motion frames")
    args=parser.parse_args()
    font=fonts()
    comparisons(Path("captures/quality"),font)
    if args.motion:
        motion(Path("captures/stability"),font)
