.PHONY: build release test verify install clean

build:
	cargo build

release:
	cargo build --release --locked

test:
	cargo test --all-targets
	cargo run -- test

verify: release
	python3 scripts/verify_repo.py target/release/oxid

install: release
	install -Dm755 target/release/oxid "$${DESTDIR}/usr/local/bin/oxid"

clean:
	cargo clean
	rm -rf .oxid
