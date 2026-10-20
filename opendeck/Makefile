PLUGIN = de.felitendo.feishin.sdPlugin
BUILD  = build/$(PLUGIN)

# Native OpenDeck. For the Flatpak use:
#   make install PLUGINS=~/.var/app/me.amankhanna.opendeck/config/opendeck/plugins
PLUGINS ?= $(HOME)/.config/opendeck/plugins

.PHONY: build install uninstall link package check clean

# The plugin for this PC, into $(BUILD).
build:
	cargo xtask build

install: build
	rm -rf "$(PLUGINS)/$(PLUGIN)"
	mkdir -p "$(PLUGINS)"
	cp -r $(BUILD) "$(PLUGINS)/$(PLUGIN)"
	@echo "Installed. Restart OpenDeck (or reload the plugin in its settings)."

uninstall:
	rm -rf "$(PLUGINS)/$(PLUGIN)"

# For development: OpenDeck runs the files from $(BUILD). Needs developer mode in OpenDeck.
link: build
	rm -rf "$(PLUGINS)/$(PLUGIN)"
	ln -s "$(CURDIR)/$(BUILD)" "$(PLUGINS)/$(PLUGIN)"

# dist/de.felitendo.feishin.streamDeckPlugin for x86_64 and aarch64, installable from the
# OpenDeck plugin page. Needs zig and cargo-zigbuild.
package:
	cargo xtask package

check:
	cargo test --workspace

clean:
	rm -rf build dist
	cargo clean
