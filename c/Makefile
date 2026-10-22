# Convenience entry points; CMake build files live under build/.
BUILD_DIR ?= build
JOBS ?= 4
PREFIX ?= $(if $(filter Linux,$(shell uname -s)),/usr,/usr/local)
.PHONY: all configure test install deps dist rpm deb source clean
all: configure
	cmake --build "$(BUILD_DIR)" --parallel $(JOBS)
configure:
	cmake -S . -B "$(BUILD_DIR)" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$(PREFIX)"
test: all
	ctest --test-dir "$(BUILD_DIR)" --output-on-failure -L smoke
install: all
	cmake --install "$(BUILD_DIR)"
deps:
	./install-deps.sh
dist:
	./packaging/build.sh auto
rpm:
	./packaging/build.sh rpm
deb:
	./packaging/build.sh deb
source:
	./packaging/build.sh source
clean:
	cmake --build "$(BUILD_DIR)" --target clean
