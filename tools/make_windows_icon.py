from PIL import Image

src = r"web\branding\eureka_app_icon.png"
dst = r"web\branding\eureka_app_icon.ico"

im = Image.open(src).convert("RGBA")
im.save(
    dst,
    sizes=[
        (16,16),
        (24,24),
        (32,32),
        (48,48),
        (64,64),
        (128,128),
        (256,256),
    ],
)

print("OK:", dst)
