PREFIX ?= $(if $(filter Linux,$(shell uname -s)),/usr,/usr/local)
DESTDIR ?=
.PHONY: all test check install deps dist source rpm deb clean
all:
	cargo build --release --locked
test:
	cargo test --locked
check:
	cargo fmt --check
	cargo clippy --locked --all-targets -- -D warnings
install: all
	install -d "$(DESTDIR)$(PREFIX)/bin"
	install -m755 target/release/spit "$(DESTDIR)$(PREFIX)/bin/spit"
	install -d "$(DESTDIR)$(PREFIX)/share/man/man1"
	install -m644 packaging/spit.1 "$(DESTDIR)$(PREFIX)/share/man/man1/spit.1"
deps:
	./install-deps.sh
dist: all test
	python3 packaging/build.py auto
rpm: all test
	python3 packaging/build.py rpm
deb: all test
	python3 packaging/build.py deb
source:
	python3 packaging/source.py
clean:
	cargo clean
