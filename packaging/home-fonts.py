"""Regenerate local Manrope instances. Requires fonttools==4.66.1; no network."""
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

root = Path(__file__).resolve().parent.parent / "assets/fonts"
for weight, name in [(400, "Regular"), (500, "Medium"), (600, "SemiBold")]:
    font = TTFont(root / "Manrope-Variable.ttf", recalcTimestamp=False)
    instantiateVariableFont(font, {"wght": weight}, inplace=True).save(root / f"Manrope-{name}.ttf")
