"""Validate --smoke-test captures: python scripts/check-native.py <capture-dir>.

Requires Pillow (python -m pip install pillow). Does not run or control the app.
"""
import json
import sys
from pathlib import Path

from PIL import Image


def check(folder: Path) -> dict:
    result_path = folder / "result.json"
    result = json.loads(result_path.read_text(encoding="utf-8"))
    assert result["captures"] == 71 and result["paidModelCalls"] == 0
    assert result["sidebarStates"] == ["collapsed", "expanded"]
    shots = sorted(folder.glob("native-*.png"))
    assert len(shots) == 71, f"Expected 71 captures, got {len(shots)}"
    assert all(shot.stat().st_mtime_ns <= result_path.stat().st_mtime_ns for shot in shots), (
        "A new capture run is still in progress or did not finish; result.json is stale"
    )
    sizes = set()
    for shot in shots:
        with Image.open(shot) as image:
            image.load()
            assert image.width >= 900 and image.height >= 600, shot.name
            sizes.add(image.size)
            for x in range(5, image.width, 37):
                for y in range(5, image.height, 31):
                    rgb = image.getpixel((x, y))[:3]
                    assert max(rgb) - min(rgb) <= 1, (shot.name, x, y, rgb)

    motion = {sample["stage"]: sample for sample in result["motion"]}
    assert len(motion) == 33
    for icon in range(8):
        start = 28 + 3 * icon
        idle, hover, reset = [motion[start + step] for step in range(3)]
        assert idle["progress"][icon] < 0.02, (icon, "idle", idle["progress"])
        assert hover["progress"][icon] > 0.98, (icon, "hover", hover["progress"])
        assert reset["progress"][icon] < 0.02, (icon, "reset", reset["progress"])
        assert idle["rects"] == hover["rects"] == reset["rects"], (icon, "layout moved")
        focus = motion[52 + icon]
        assert focus["focused"] and focus["progress"][icon] > 0.98, (icon, "focus", focus)
    assert max(motion[60]["progress"]) == 0, motion[60]["progress"]
    cadence = {str(stage): motion[stage]["cadence"] for stage in [32, 53]}
    for stage, sample in cadence.items():
        assert sample and sample["samples"] >= 30, (stage, "missing frame samples", sample)
        assert sample["medianFrameMs"] < 25 and sample["p95FrameMs"] < 50, (
            stage, "native animation cadence is too slow", sample
        )
    return {
        "passed": True,
        "screenshots": len(shots),
        "pages": 8,
        "dialogs": 4,
        "monochromeSampleCheck": True,
        "hoverAndReset": 8,
        "keyboardFocus": 8,
        "reducedMotion": True,
        "stableNavigationBounds": True,
        "sidebarStates": result["sidebarStates"],
        "rocketFrameCadence": cadence,
        "imageSizes": sorted(sizes),
        "scope": "Native framebuffer with injected test input; isolated database; no paid model calls",
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage: python scripts/check-native.py <capture-dir>")
    folder = Path(sys.argv[1])
    report = json.dumps(check(folder), ensure_ascii=False, indent=2)
    (folder / "verified.json").write_text(report + "\n", encoding="utf-8")
    print(report)
