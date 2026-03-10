CARGO ?= $(HOME)/.cargo/bin/cargo

.PHONY: check run

check:
	$(CARGO) check

run:
	$(CARGO) run
