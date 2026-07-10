import subprocess
import sys
import os
import re

def run_cmd(cmd, cwd=None):
    print(f"Running: {cmd} (in {cwd or '.'})")
    res = subprocess.run(cmd, shell=True, cwd=cwd, text=True, capture_output=True)
    if res.returncode != 0:
        print(f"Error: {res.stderr}")
        sys.exit(res.returncode)
    return res.stdout.strip()

def main():
    if len(sys.argv) < 2:
        print("Usage: python scripts/bump.py [patch|minor|major]")
        sys.exit(1)
    
    part = sys.argv[1]
    
    # 1. Read current version from Cargo.toml
    with open("Cargo.toml", "r", encoding="utf-8") as f:
        content = f.read()
    match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
    if not match:
        print("Could not find current version in Cargo.toml")
        sys.exit(1)
    current_version = match.group(1)
    print(f"Current version: {current_version}")
    
    # 2. Run bumpversion to update all files on disk
    print("Running bumpversion...")
    run_cmd(f"python -m bumpversion --verbose {part}")
    
    # 3. Read the new version from Cargo.toml
    with open("Cargo.toml", "r", encoding="utf-8") as f:
        content = f.read()
    match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
    if not match:
        print("Could not find new version in Cargo.toml")
        sys.exit(1)
    new_version = match.group(1)
    print(f"New version: {new_version}")
    
    # 4. Commit and push inside crates/forja-wasm submodule
    print("\n--- Updating crates/forja-wasm submodule ---")
    run_cmd("git add Cargo.toml", cwd="crates/forja-wasm")
    run_cmd(f'git commit -m "Bump version: {current_version} -> {new_version}"', cwd="crates/forja-wasm")
    run_cmd("git push origin main", cwd="crates/forja-wasm")
    
    # 5. Commit and push inside vscode submodule
    print("\n--- Updating vscode submodule ---")
    run_cmd("git add forja-syntax/package.json forja-syntax/package-lock.json", cwd="vscode")
    run_cmd(f'git commit -m "Bump version: {current_version} -> {new_version}"', cwd="vscode")
    run_cmd("git push origin main", cwd="vscode")
    
    # 6. Commit, tag and push in root repository
    print("\n--- Updating root repository ---")
    run_cmd("git add Cargo.toml src/main.rs docs/src/pages/roadmap.astro benchmarks/RESULTADOS_BENCHMARK.md benchmarks/README.md crates/forja-wasm vscode .bumpversion.cfg")
    run_cmd(f'git commit -m "Bump version: {current_version} -> {new_version}"')
    run_cmd(f"git tag v{new_version}")
    run_cmd("git push origin main --tags")
    
    print(f"\nSuccessfully bumped and pushed version {new_version} for everything!")

if __name__ == "__main__":
    main()
