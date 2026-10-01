#!/usr/bin/env python3
"""Run the README walkthrough in a disposable project with a selected Sirno binary."""

import argparse
from pathlib import Path
import re
import shlex
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, help="path to the built sirno executable")
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"binary not found: {binary}")

    readme = (Path(__file__).resolve().parent.parent / "README.md").read_text()
    begin = "<!-- sirno:quickstart:begin -->"
    end = "<!-- sirno:quickstart:end -->"
    if readme.count(begin) != 1 or readme.count(end) != 1 or readme.index(begin) >= readme.index(end):
        raise SystemExit("README must contain one ordered pair of quick-start markers")
    walkthrough = readme.split(begin, 1)[1].split(end, 1)[0]
    blocks = re.findall(r"^```sh\n(.*?)\n```$", walkthrough, re.MULTILINE | re.DOTALL)
    expected = re.findall(r"^```text\n(.*?)\n```$", walkthrough, re.MULTILINE | re.DOTALL)
    if not blocks or not expected or not any(output.startswith("status clean:") for output in expected):
        raise SystemExit("README walkthrough must include shell blocks and a clean-status output")

    script = "\n".join(blocks) + "\n"
    pending = "sirno tide status"
    if script.splitlines().count(pending) != 1:
        raise SystemExit("README walkthrough must inspect pending Tide exactly once")
    # The published commands run unchanged; this guard checks the documented exit code.
    script = script.replace(
        f"\n{pending}\n",
        f"\nif {pending}; then\n"
        "  echo 'expected pending Tide to exit 1' >&2\n"
        "  exit 1\n"
        "else\n"
        '  test "$?" -eq 1\n'
        "fi\n",
    )
    script = f'sirno() {{ {shlex.quote(str(binary))} "$@"; }}\n' + script
    with tempfile.TemporaryDirectory(prefix="sirno-readme-") as directory:
        result = subprocess.run(
            ["sh", "-eu"], input=script, text=True, cwd=directory,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60,
        )
    if result.returncode != 0:
        raise SystemExit(f"README walkthrough failed (exit {result.returncode}):\n{result.stdout}")
    expected.extend([
        "The tide has 1 open workitem in 1 wave, with 1 unique review entry.",
        'console.log("Hello, world.");',
        'console.log("Hello, Sirno.");',
    ])
    for output in expected:
        if output not in result.stdout:
            raise SystemExit(f"README output missing: {output!r}\n{result.stdout}")
    print(f"README smoke check passed: {len(blocks)} shell blocks, pending review, and clean completion.")


if __name__ == "__main__":
    main()
