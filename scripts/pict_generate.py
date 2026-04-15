#!/usr/bin/env python3
"""Generate pairwise cases from a PICT model with a single agent-friendly entry.

Backends:
1. Local `pict` CLI
2. `python -m pypict`
3. Docker image with PICT

Examples:
  python3 scripts/pict_generate.py model.txt
  python3 scripts/pict_generate.py model.txt --format json
  python3 scripts/pict_generate.py model.txt --backend docker --docker-build-if-missing
  python3 scripts/pict_generate.py --self-test
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys
import unittest
from dataclasses import dataclass


DEFAULT_DELIMITER = ","
DEFAULT_DOCKER_IMAGE = "pict:latest"
DEFAULT_DOCKER_BUILD_URL = "https://github.com/microsoft/pict.git"
DEFAULT_DOCKER_BUILD_FILE = "Containerfile"


@dataclass
class GenerationResult:
    backend: str
    headers: list[str]
    rows: list[list[str]]
    raw_output: str


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Generate pairwise test cases from a PICT model.",
    )
    parser.add_argument("model", nargs="?", help="Path to the PICT model file")
    parser.add_argument("--backend", choices=["auto", "pict", "pypict", "docker"], default="auto")
    parser.add_argument("--order", type=int, default=2, help="Combination order, defaults to 2")
    parser.add_argument("--format", choices=["markdown", "json", "raw", "tsv"], default="markdown")
    parser.add_argument("--delimiter", default=DEFAULT_DELIMITER, help="PICT output delimiter")
    parser.add_argument("--output", help="Write formatted output to a file instead of stdout")
    parser.add_argument("--docker-image", default=DEFAULT_DOCKER_IMAGE, help="Docker image used for docker backend")
    parser.add_argument(
        "--docker-build-if-missing",
        action="store_true",
        help="Build the Docker image from the upstream PICT repo when it is missing",
    )
    parser.add_argument(
        "--docker-build-url",
        default=DEFAULT_DOCKER_BUILD_URL,
        help="Remote Git URL used when building the Docker image",
    )
    parser.add_argument(
        "--docker-build-file",
        default=DEFAULT_DOCKER_BUILD_FILE,
        help="Dockerfile or Containerfile path used when building the Docker image",
    )
    parser.add_argument(
        "--pict-arg",
        action="append",
        default=[],
        help="Extra argument forwarded to the selected PICT backend; can be repeated",
    )
    parser.add_argument("--self-test", action="store_true", help="Run script unit tests")
    return parser


def command_exists(name: str) -> bool:
    return shutil.which(name) is not None


def pypict_available() -> bool:
    return importlib.util.find_spec("pypict") is not None


def run_process(command: list[str]) -> str:
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode == 0:
        return result.stdout

    stderr = result.stderr.strip()
    stdout = result.stdout.strip()
    detail = stderr or stdout or "unknown error"
    raise RuntimeError(f"Command failed ({' '.join(command)}): {detail}")


def pict_args(order: int, delimiter: str, extra_args: list[str]) -> list[str]:
    return [f"/o:{order}", f"/d:{delimiter}", *extra_args]


def run_cli_backend(executable: list[str], model_path: pathlib.Path, order: int, delimiter: str, extra_args: list[str]) -> str:
    command = [*executable, str(model_path), *pict_args(order, delimiter, extra_args)]
    return run_process(command)


def docker_image_exists(image: str) -> bool:
    result = subprocess.run(
        ["docker", "image", "inspect", image],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def docker_build_command(image: str, build_url: str, build_file: str) -> list[str]:
    return ["docker", "build", "-f", build_file, "-t", image, build_url]


def ensure_docker_image(image: str, build_if_missing: bool, build_url: str, build_file: str) -> None:
    if docker_image_exists(image):
        return
    if not build_if_missing:
        raise RuntimeError(
            f"Docker image '{image}' is missing. Re-run with --docker-build-if-missing or pre-build the image."
        )

    command = docker_build_command(image, build_url, build_file)
    run_process(command)


def run_docker_backend(
    model_path: pathlib.Path,
    order: int,
    delimiter: str,
    extra_args: list[str],
    image: str,
    build_if_missing: bool,
    build_url: str,
    build_file: str,
) -> str:
    if not command_exists("docker"):
        raise RuntimeError("Docker backend requested, but docker is not installed")

    ensure_docker_image(image, build_if_missing, build_url, build_file)
    parent = str(model_path.parent.resolve())
    mounted_model = f"/var/pict/{model_path.name}"
    command = [
        "docker",
        "run",
        "--rm",
        "-v",
        f"{parent}:/var/pict",
        image,
        mounted_model,
        *pict_args(order, delimiter, extra_args),
    ]
    return run_process(command)


def choose_backend(requested: str, docker_image: str, build_if_missing: bool) -> str:
    if requested != "auto":
        return requested
    if command_exists("pict"):
        return "pict"
    if pypict_available():
        return "pypict"
    if command_exists("docker") and (docker_image_exists(docker_image) or build_if_missing):
        return "docker"
    raise RuntimeError(
        "No usable PICT backend found. Install `pict`, install `pypict`, or use --backend docker --docker-build-if-missing."
    )


def split_pict_line(line: str, delimiter: str) -> list[str]:
    if delimiter and delimiter in line:
        return [part.strip() for part in line.split(delimiter)]
    if "\t" in line:
        return [part.strip() for part in line.split("\t")]
    return [line.strip()]


def parse_pict_output(output: str, delimiter: str) -> tuple[list[str], list[list[str]]]:
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError("PICT produced no output")

    headers = split_pict_line(lines[0], delimiter)
    rows: list[list[str]] = []
    for line in lines[1:]:
        row = split_pict_line(line, delimiter)
        if len(row) != len(headers):
            raise RuntimeError(f"Unexpected PICT output row: {line}")
        rows.append(row)
    return headers, rows


def render_markdown(result: GenerationResult) -> str:
    header_line = "| " + " | ".join(result.headers) + " |"
    separator_line = "| " + " | ".join(["---"] * len(result.headers)) + " |"
    body_lines = ["| " + " | ".join(row) + " |" for row in result.rows]
    lines = [
        "## PICT Generated Cases",
        "",
        f"- Backend: {result.backend}",
        f"- Cases: {len(result.rows)}",
        "",
        header_line,
        separator_line,
        *body_lines,
    ]
    return "\n".join(lines)


def render_json(result: GenerationResult) -> str:
    payload = {
        "backend": result.backend,
        "headers": result.headers,
        "rows": result.rows,
        "case_count": len(result.rows),
    }
    return json.dumps(payload, indent=2, ensure_ascii=False)


def render_tsv(result: GenerationResult) -> str:
    lines = ["\t".join(result.headers)]
    lines.extend("\t".join(row) for row in result.rows)
    return "\n".join(lines)


def format_result(result: GenerationResult, output_format: str) -> str:
    if output_format == "raw":
        return result.raw_output
    if output_format == "json":
        return render_json(result)
    if output_format == "tsv":
        return render_tsv(result)
    return render_markdown(result)


def write_output(text: str, destination: str | None) -> None:
    if destination is None:
        print(text)
        return
    output_path = pathlib.Path(destination)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(text, encoding="utf-8")


def generate_cases(args: argparse.Namespace) -> GenerationResult:
    if not args.model:
        raise RuntimeError("Model path is required unless --self-test is used")

    model_path = pathlib.Path(args.model).resolve()
    if not model_path.is_file():
        raise RuntimeError(f"Model file not found: {model_path}")

    backend = choose_backend(args.backend, args.docker_image, args.docker_build_if_missing)
    if backend == "pict":
        output = run_cli_backend(["pict"], model_path, args.order, args.delimiter, args.pict_arg)
    elif backend == "pypict":
        output = run_cli_backend([sys.executable, "-m", "pypict"], model_path, args.order, args.delimiter, args.pict_arg)
    else:
        output = run_docker_backend(
            model_path,
            args.order,
            args.delimiter,
            args.pict_arg,
            args.docker_image,
            args.docker_build_if_missing,
            args.docker_build_url,
            args.docker_build_file,
        )

    headers, rows = parse_pict_output(output, args.delimiter)
    return GenerationResult(backend=backend, headers=headers, rows=rows, raw_output=output)


class PictGenerateTests(unittest.TestCase):
    def test_parse_pict_output(self) -> None:
        output = "A;B\n1;2\n3;4\n"
        headers, rows = parse_pict_output(output, ";")
        self.assertEqual(headers, ["A", "B"])
        self.assertEqual(rows, [["1", "2"], ["3", "4"]])

    def test_render_markdown(self) -> None:
        result = GenerationResult("pict", ["A", "B"], [["1", "2"]], "A;B\n1;2")
        markdown = render_markdown(result)
        self.assertIn("Backend: pict", markdown)
        self.assertIn("| A | B |", markdown)
        self.assertIn("| 1 | 2 |", markdown)

    def test_parse_pict_output_falls_back_to_tabs(self) -> None:
        output = "A\tB\n1\t2\n3\t4\n"
        headers, rows = parse_pict_output(output, ",")
        self.assertEqual(headers, ["A", "B"])
        self.assertEqual(rows, [["1", "2"], ["3", "4"]])

    def test_choose_backend_prefers_pict(self) -> None:
        original_command_exists = globals()["command_exists"]
        original_pypict_available = globals()["pypict_available"]
        original_docker_image_exists = globals()["docker_image_exists"]
        try:
            globals()["command_exists"] = lambda name: name == "pict"
            globals()["pypict_available"] = lambda: True
            globals()["docker_image_exists"] = lambda image: True
            self.assertEqual(choose_backend("auto", DEFAULT_DOCKER_IMAGE, False), "pict")
        finally:
            globals()["command_exists"] = original_command_exists
            globals()["pypict_available"] = original_pypict_available
            globals()["docker_image_exists"] = original_docker_image_exists

    def test_docker_build_command_uses_containerfile(self) -> None:
        command = docker_build_command(
            "pict:test",
            "https://github.com/microsoft/pict.git",
            "Containerfile",
        )
        self.assertEqual(
            command,
            [
                "docker",
                "build",
                "-f",
                "Containerfile",
                "-t",
                "pict:test",
                "https://github.com/microsoft/pict.git",
            ],
        )


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(PictGenerateTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    try:
        result = generate_cases(args)
        write_output(format_result(result, args.format), args.output)
        return 0
    except RuntimeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())