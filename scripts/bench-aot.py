#!/usr/bin/env python3
"""Mide AOT nativo de Forja contra Rust y el AOT de bytecode.

Uso: python scripts/bench-aot.py [--runs 5]
"""
import argparse, os, statistics, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORJA = ROOT / "target" / "release" / ("forja.exe" if os.name == "nt" else "forja")
FIXTURE = ROOT / "benches" / "fixtures" / "fib42.fa"


def medir(exe: Path, runs: int) -> float:
    for _ in range(2):
        subprocess.run([str(exe)], check=True, stdout=subprocess.DEVNULL)
    return statistics.median(
        (lambda t: (subprocess.run([str(exe)], check=True, stdout=subprocess.DEVNULL), (time.perf_counter() - t) * 1000)[1])(time.perf_counter())
        for _ in range(runs)
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=5)
    args = ap.parse_args()
    with tempfile.TemporaryDirectory(prefix="forja_aot_") as td:
        td = Path(td)
        native = td / ("fib_native.exe" if os.name == "nt" else "fib_native")
        subprocess.run([str(FORJA), "aot", str(FIXTURE), str(native)], check=True,
                       stdout=subprocess.DEVNULL)
        rust_src = td / "fib.rs"
        rust_src.write_text(
            "fn fib(n:u64)->u64{if n<=1{return n;}fib(n-1)+fib(n-2)}\n"
            "fn main(){println!(\"{}\", fib(42));}\n", encoding="utf-8")
        rust = td / ("fib_rust.exe" if os.name == "nt" else "fib_rust")
        subprocess.run(["rustc", "-O", str(rust_src), "-o", str(rust)], check=True)
        print(f"AOT nativo : {medir(native, args.runs):.2f} ms")
        print(f"Rust       : {medir(rust, args.runs):.2f} ms")
        print(f"tamaño AOT : {native.stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
