"""Regenera docs/assets/loomcoding-logo.png: el logotipo L∞MCODING en píxel gordo.

Cabe señalar que el dibujo se hace a resolución de «unidad» (4 px) y se amplía sin suavizar, de modo
que el aspecto de píxel sale solo y el resultado es determinista. Requiere Pillow. Uso:

    python scripts/capturas/logo.py
"""

import math
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

RAIZ = Path(__file__).resolve().parents[2]
DESTINO = RAIZ / "docs" / "assets" / "loomcoding-logo.png"

U = 4  # píxeles por unidad
ANCHO, ALTO = 550, 140
LETRA_W, LETRA_H, TRAZO, HUECO = 9, 12, 2, 2
FONDO = (1, 8, 1)
VERDE = (15, 245, 75)
BRILLO = (0, 255, 65)


def rect(d, x0, y0, x1, y1, valor=1):
    d.rectangle((x0, y0, x1, y1), fill=valor)


def letra(nombre: str) -> Image.Image:
    """Máscara 1 bit de una letra de LETRA_W × LETRA_H unidades."""
    w, h, s = LETRA_W, LETRA_H, TRAZO
    img = Image.new("1", (w, h), 0)
    d = ImageDraw.Draw(img)
    if nombre == "L":
        rect(d, 0, 0, s - 1, h - 1)
        rect(d, 0, h - s, w - 1, h - 1)
    elif nombre == "I":
        rect(d, 0, 0, w - 1, s - 1)
        rect(d, 0, h - s, w - 1, h - 1)
        rect(d, w // 2 - 1, 0, w // 2, h - 1)
    elif nombre == "M":
        rect(d, 0, 0, s - 1, h - 1)
        rect(d, w - s, 0, w - 1, h - 1)
        d.line((1, 0, 4, 6), fill=1, width=s)
        d.line((w - 2, 0, 4, 6), fill=1, width=s)
    elif nombre == "N":
        rect(d, 0, 0, s - 1, h - 1)
        rect(d, w - s, 0, w - 1, h - 1)
        d.line((1, 0, w - 2, h - 1), fill=1, width=s)
    elif nombre in ("O", "C", "G"):
        d.rounded_rectangle((0, 0, w - 1, h - 1), radius=4, outline=1, width=s)
        if nombre in ("C", "G"):
            rect(d, w - 4, 2, w - 1, h - 3, 0)
        if nombre == "G":
            rect(d, w // 2, h // 2, w - 1, h // 2 + 1)
            rect(d, w - 2, h // 2, w - 1, h - 3)
    elif nombre == "D":
        d.rounded_rectangle(
            (0, 0, w - 1, h - 1), radius=4, outline=1, width=s, corners=(False, True, True, False)
        )
        rect(d, 0, 0, s - 1, h - 1)
    else:
        raise ValueError(nombre)
    return img


def infinito(ancho: int = 22, alto: int = LETRA_H) -> Image.Image:
    """El ∞ que sustituye a las dos O de LOOM: una lemniscata de Bernoulli."""
    img = Image.new("1", (ancho, alto), 0)
    d = ImageDraw.Draw(img)
    a = (ancho - TRAZO) / 2
    b = (alto - TRAZO) / 2 / 0.3536  # 0.3536 es el máximo de sen·cos/(1+sen²)
    cx, cy = (ancho - 1) / 2, (alto - 1) / 2
    puntos = []
    for i in range(0, 361):
        t = math.radians(i)
        k = 1 + math.sin(t) ** 2
        puntos.append((cx + a * math.cos(t) / k, cy + b * math.sin(t) * math.cos(t) / k))
    d.line(puntos, fill=1, width=TRAZO, joint="curve")
    return img


def componer_palabra() -> Image.Image:
    piezas = [letra("L"), infinito(), letra("M")] + [letra(c) for c in "CODING"]
    total = sum(p.width for p in piezas) + HUECO * (len(piezas) - 1)
    lienzo = Image.new("1", (total, LETRA_H), 0)
    x = 0
    for pieza in piezas:
        lienzo.paste(pieza, (x, 0))
        x += pieza.width + HUECO
    return lienzo


def decoracion(capa: Image.Image) -> None:
    """Columnas de puntos a la izquierda, una guía a la derecha y una regla inferior, como el original."""
    d = ImageDraw.Draw(capa)
    for n, x in enumerate((7, 13, 19, 25)):
        for k, y in enumerate(range(2, ALTO - 2, 9)):
            g = 40 + ((k * 37 + n * 53) % 80)
            d.rectangle((x, y, x + 2, y + 3), fill=(0, g, 0))
    for y in range(60, 130, 8):
        d.rectangle((522, y, 523, y + 3), fill=(0, 90, 20))
        d.rectangle((528, y + 4, 529, y + 6), fill=(0, 60, 14))
    d.rectangle((7, ALTO - 4, 524, ALTO - 3), fill=(0, 70, 18))
    for x in range(40, 520, 60):
        d.rectangle((x, ALTO - 10, x + 1, ALTO - 4), fill=(0, 70, 18))


def fuente() -> ImageFont.FreeTypeFont:
    for candidata in ("DejaVuSansMono.ttf", "consola.ttf", "Courier New.ttf"):
        try:
            return ImageFont.truetype(candidata, 21)
        except OSError:
            continue
    raise SystemExit("No se encuentra una fuente monoespaciada (DejaVu Sans Mono o Consolas).")


def main() -> None:
    palabra = componer_palabra()
    mascara = palabra.resize((palabra.width * U, palabra.height * U), Image.NEAREST).convert("L")
    mascara = mascara.point(lambda v: 255 if v else 0)
    lienzo = Image.new("RGB", (ANCHO, ALTO), FONDO)
    decoracion(lienzo)
    x0 = (ANCHO - mascara.width) // 2 + 14
    y0 = 12
    brillo = Image.new("L", (ANCHO, ALTO), 0)
    brillo.paste(mascara, (x0, y0))
    halo = brillo.filter(ImageFilter.GaussianBlur(7)).point(lambda v: int(v * 0.55))
    lienzo = Image.composite(Image.new("RGB", (ANCHO, ALTO), BRILLO), lienzo, halo)
    lienzo.paste(Image.new("RGB", mascara.size, VERDE), (x0, y0), mascara)
    texto = "--- weaving signals from code ---"
    d = ImageDraw.Draw(lienzo)
    f = fuente()
    caja = d.textbbox((0, 0), texto, font=f)
    d.text(((ANCHO - (caja[2] - caja[0])) // 2 + 14, 88), texto, font=f, fill=VERDE)
    lienzo.save(DESTINO, optimize=True)
    print(f"Logotipo regenerado: {DESTINO} ({DESTINO.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
