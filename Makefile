PYTHON_LIB = target/python-dev/python/lib/libpython3.11.dylib

.PHONY: build setup clean

build: setup
	cargo xtask build --release
	cp target/release/gen-audiobook gen-audio

setup: $(PYTHON_LIB)
	@# Fix install_name if not already fixed
	@if otool -D $(PYTHON_LIB) | grep -q '/install/lib'; then \
		echo "Fixing Python library install_name..."; \
		install_name_tool -id @rpath/libpython3.11.dylib $(PYTHON_LIB); \
	fi

$(PYTHON_LIB):
	cargo xtask setup

clean:
	cargo clean
	rm -f gen-audio
