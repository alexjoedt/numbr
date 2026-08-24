.PHONY: all build clean install uninstall

all: build

build:
	cargo build

release:
	cargo build --release

clean:
	cargo clean

install: build
	mkdir -p ~/.local/bin
	cp target/debug/numbr ~/.local/bin/numbr

uninstall:
	rm -f ~/.local/bin/numbr