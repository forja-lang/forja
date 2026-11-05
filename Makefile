# Forja — tareas de verificación
#
# `make verify` es equivalente a `bash scripts/verify.sh`.
.PHONY: verify fmt clippy test metricas

verify:
	bash scripts/verify.sh

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --locked

test:
	cargo test --locked

metricas:
	python3 scripts/metricas.py --check
