#!/usr/bin/env python3
"""Mide fib(42) en binario Forja ASM vs binario Rust.

Uso:
  python scripts/bench-fib-asm.py [--runs 5]

No cuenta compilación: compila una vez, ejecuta N veces y reporta mediana.
"""
import argparse, os, statistics, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORJA = ROOT / "target" / "release" / ("forja.exe" if os.name == "nt" else "forja")
FIXTURE = ROOT / "benches" / "fixtures" / "fib42.fa"


def run_once(exe: Path) -> float:
    t = time.perf_counter()
    subprocess.run([str(exe)], check=True, stdout=subprocess.DEVNULL)
    return (time.perf_counter() - t) * 1000.0


def medir(exe: Path, runs: int) -> float:
    for _ in range(2):
        run_once(exe)
    return statistics.median(run_once(exe) for _ in range(runs))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=5)
    args = ap.parse_args()

    if not FORJA.is_file():
        print(f"falta {FORJA}; ejecutá cargo build --release --bin forja", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="forja_fib_") as td:
        td = Path(td)
        asm_base = td / "fib_asm"
        subprocess.run([str(FORJA), "asm", str(FIXTURE), str(asm_base)], check=True,
                       stdout=subprocess.DEVNULL)
        asm_exe = asm_base.with_suffix(".exe") if os.name == "nt" else asm_base

        rust_src = td / "fib.rs"
        rust_src.write_text(
            "fn fib(n:u64)->u64{if n<=1{return n;}fib(n-1)+fib(n-2)}\n"
            "fn main(){println!(\"{}\", fib(42));}\n",
            encoding="utf-8",
        )
        rust_exe = td / ("fib_rust.exe" if os.name == "nt" else "fib_rust")
        subprocess.run(["rustc", "-O", str(rust_src), "-o", str(rust_exe)], check=True)

        asm_ms = medir(asm_exe, args.runs)
        rust_ms = medir(rust_exe, args.runs)
        print(f"Forja ASM : {asm_ms:.2f} ms (mediana de {args.runs})")
        print(f"Rust      : {rust_ms:.2f} ms (mediana de {args.runs})")
        print(f"ratio     : {asm_ms / rust_ms:.2f}x")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
