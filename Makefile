CARGO ?= $(HOME)/.cargo/bin/cargo

.PHONY: check run format

check:
	$(CARGO) check

run:
	$(CARGO) run

format:
	$(CARGO) fmt
