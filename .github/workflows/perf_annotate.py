import argparse
import json
from os import environ
from pathlib import Path
from typing import List, Any, Dict, cast


class Args(argparse.Namespace):
    json_file: Path


def main():
    arg_parser = argparse.ArgumentParser()
    arg_parser.add_argument("json_file", type=Path)
    arg_parser.parse_args(namespace=Args)

    bench_json = Args.json_file.read_text(encoding="utf-8")
    bench: List[Dict[str, Any]] = json.loads(bench_json)["results"]

    assert len(bench) == 3
    old_mean, new_mean = (None, None)
    for result in bench:
        mean = cast(float, result["mean"])
        if result["command"] == "previous-build":
            old_mean = mean
        elif result["command"] == "current-build":
            new_mean = mean

    assert old_mean is not None, "benchmark report has no result for previous-build"
    assert new_mean is not None, "benchmark report has no result for current-build"

    diff = round(new_mean - old_mean, 2)
    scalar = int((new_mean - old_mean) / old_mean * 100)

    output = []
    if diff > 2:
        output.extend(
            [
                "> [!CAUTION]",
                "> Detected a performance regression in new changes:",
            ]
        )
    elif diff < -2:
        output.extend(
            [
                "> [!TIP]",
                "> Detected a performance improvement in new changes:",
            ]
        )
    else:
        output.extend(
            [
                "> [!NOTE]",
                "> Determined a negligible difference in performance with new changes:",
            ]
        )
    output[-1] += f" {diff}s ({scalar} %)"
    annotation = "\n".join(output)

    if "GITHUB_STEP_SUMMARY" in environ:
        with open(environ["GITHUB_STEP_SUMMARY"], "a") as summary:
            summary.write(f"\n{annotation}\n")
    else:
        print(annotation)


if __name__ == "__main__":
    main()
